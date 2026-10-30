// Ported from effect-tsgo's internal/rules/api_stability_leak_regression_test.go. The program helpers keep Go's
// shape; `ApiStabilityCompiled` stands for Go's `(*rule.Context, done)` pair (see there).

use std::fmt::Write as _;
use std::mem::ManuallyDrop;
use std::sync::{Arc, Once};

use ets_etscore::{ResolvedEffectPluginOptions, Severity};
use ets_go::testing::{self, T};
use ets_rule::Context;
use ets_shim::P;
use ets_shim::ast::{Diagnostic, SourceFile};
use ets_shim::checker::TypeFlags;
use ets_shim::compiler::{CheckerHandle, Program, ProgramOptions, ProgramRegions};
use ets_shim::core::arena::Region;
use ets_shim::core::context::Context as GoContext;
use ets_shim::core::tspath::ComparePathsOptions;
use ets_shim::core::{
    CompilerOptions, ModuleKind, ModuleResolutionKind, NewLineKind, ScriptTarget, Tristate,
};
use ets_shim::vfs::vfstest::MapFile;
use ets_shim::vfs::{FS, bundled, vfstest};
use ets_typeparser::{
    ApiStabilityUsed, api_stability_dependency_name, get_conditional_type_branch_node,
    get_declared_index_infos_of_symbol, get_resolved_conditional_type_branch,
    get_resolved_declared_type_of_symbol_if_materialized, get_resolved_type_from_type_node,
    get_resolved_type_of_symbol_if_materialized, new_type_parser, record_checked_expression_type,
};

use crate::*;

/// Rust-only: the Go test binary of `internal/rules` imports `etscheckerhooks` (`rules_json_test.go`), whose `init`
/// registers `typeparser.RecordCheckedExpressionType` as the checker's expression-checked callback; every
/// program-building helper here runs this once before compiling. (The after-check callback it also registers runs
/// no rule for these programs, which carry no Effect plugin options.)
fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        ets_shim::checker::register_expression_checked_callback(record_checked_expression_type);
    });
}

/// Rust-only: Go's `map[string]string` of test files (name relative to `/.src/` -> content).
pub(crate) type Files = Vec<(String, String)>;

/// Rust-only: builds `Files` from string pairs (Go's map literal).
pub(crate) fn files(entries: &[(&str, &str)]) -> Files {
    entries.iter().map(|&(name, content)| (name.to_string(), content.to_string())).collect()
}

/// Rust-only: Go's string concatenation (`a + b + c`) of the fixtures.
pub(crate) fn cat(parts: &[&str]) -> String {
    parts.concat()
}

/// Rust-only: runs a test body on a thread with the checker's stack (`ets_go::testing::on_checker_stack`).
pub(crate) fn run(f: impl FnOnce(&T) + Send + 'static) {
    testing::on_checker_stack(move || testing::run(f));
}

/// Rust-only: Go's `(*rule.Context, done)` pair. The rule context borrows a type parser, which borrows the checker,
/// so it cannot be returned: `with_rule_ctx` lends it to a closure (one type parser for the value's lifetime, as Go's
/// context holds one), and dropping the value is Go's `done()`, which also frees the program as Go's GC does
/// (`porting/TSRS_API.md`, "The API path", step 6).
pub(crate) struct ApiStabilityCompiled {
    pub(crate) program: &'static Program,
    pub(crate) source_file: P<SourceFile>,
    // Go: `ctx.Options`, nil unless a test assigns it.
    pub(crate) options: Option<ResolvedEffectPluginOptions>,
    checker: ManuallyDrop<CheckerHandle>,
    _loader_regions: Arc<ProgramRegions>,
    _region: Region,
}

impl ApiStabilityCompiled {
    /// Rust-only: runs `f` with Go's `rule.NewContext(ctx, program, c, tp, sf, nil, etscore.SeverityWarning)`.
    pub(crate) fn with_rule_ctx<R>(&mut self, f: impl FnOnce(&mut Context<'_, '_>) -> R) -> R {
        let ctx = GoContext::background();
        let program = self.program;
        let source_file = self.source_file;
        let mut tp = new_type_parser(program, &mut self.checker);
        let mut rule_ctx = ets_rule::new_context(
            &ctx,
            program,
            &mut tp,
            source_file,
            self.options.as_ref(),
            Severity::Warning,
        );
        f(&mut rule_ctx)
    }
}

impl Drop for ApiStabilityCompiled {
    fn drop(&mut self) {
        // SAFETY: the handle is dropped once, here, and not used afterwards.
        unsafe { ManuallyDrop::drop(&mut self.checker) };
        // SAFETY: a program from `new_program` that never shared data with another version; the test is done with
        // everything derived from it (`with_rule_ctx` borrows this value).
        unsafe { ets_shim::compiler::free_unshared_program(self.program) };
    }
}

// go test: internal/rules/api_stability_leak_regression_test.go:24 compileApiStabilityFiles bcef30d02701c71f
pub(crate) fn compile_api_stability_files(t: &T, files: &Files) -> ApiStabilityCompiled {
    t.helper();
    compile_api_stability_files_primed(t, files, true)
}

// go test: internal/rules/api_stability_leak_regression_test.go:29 compileApiStabilityFilesPrimed 38af34ff1f95306a
pub(crate) fn compile_api_stability_files_primed(
    t: &T,
    files: &Files,
    prime: bool,
) -> ApiStabilityCompiled {
    t.helper();
    init();

    let mut testfs = Vec::with_capacity(files.len());
    let mut file_names = Vec::with_capacity(files.len());
    for (name, content) in files {
        let path = format!("/.src/{name}");
        testfs.push((path.clone(), MapFile::from(content.as_str())));
        file_names.push(path);
    }
    file_names.sort();

    let region = Region::new_scratch(1 << 20);
    let loader_regions = Arc::new(ProgramRegions::default());
    let program = {
        let _scope = region.enter();
        let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(testfs, true)));

        let compiler_options = CompilerOptions {
            new_line: NewLineKind::LF,
            skip_default_lib_check: Tristate::True,
            skip_lib_check: Tristate::True,
            no_error_truncation: Tristate::True,
            target: ScriptTarget::ES_NEXT,
            module: ModuleKind::NODE_NEXT,
            module_resolution: ModuleResolutionKind::NODE_NEXT,
            strict: Tristate::True,
            ..Default::default()
        };
        let host =
            ets_shim::compiler::new_compiler_host("/", fs, &bundled::lib_path(), None, None, None);
        let config = ets_shim::tsoptions::new_parsed_command_line(
            P::new(compiler_options),
            file_names,
            Vec::new(),
            ComparePathsOptions {
                use_case_sensitive_file_names: true,
                current_directory: "/.src".into(),
            },
        );
        let mut options = ProgramOptions::new(P::new(config), host);
        options.single_threaded = Tristate::True;
        options.checker_regions = true;
        options.checker_diagnostic_regions = true;
        options.regions = Some(Arc::clone(&loader_regions));
        ets_shim::compiler::new_program(options)
    };

    let ctx = GoContext::background();
    if prime {
        let _ = program.get_semantic_diagnostics(&ctx, None);
    }
    let checker = program.get_type_checker(&ctx);
    let Some(sf) = program.get_source_file("/.src/test.ts") else {
        drop(checker);
        t.fatal("failed to get source file");
    };
    ApiStabilityCompiled {
        program,
        source_file: sf,
        options: None,
        checker: ManuallyDrop::new(checker),
        _loader_regions: loader_regions,
        _region: region,
    }
}

// go test: internal/rules/api_stability_leak_regression_test.go:76 runApiStabilityMessages b7a9cacc63996f48
pub(crate) fn run_api_stability_messages(t: &T, rule_ctx: &mut Context<'_, '_>) -> Vec<String> {
    t.helper();
    let diagnostics = (API_STABILITY_LEAK.run)(rule_ctx);
    let mut messages = Vec::with_capacity(diagnostics.len());
    for diagnostic in &diagnostics {
        messages.push(diagnostic.localize());
    }
    messages
}

// go test: internal/rules/api_stability_leak_regression_test.go:86 containsApiStabilityMessage 623ef286624fa5b0
pub(crate) fn contains_api_stability_message(t: &T, messages: &[String], want: &str) -> bool {
    t.helper();
    messages.iter().any(|message| message.contains(want))
}

// go test: internal/rules/api_stability_leak_regression_test.go:93 apiStabilityCompilerDiagnostics fcd920d9016abd70
pub(crate) fn api_stability_compiler_diagnostics(rule_ctx: &mut Context<'_, '_>) -> Vec<String> {
    let mut out = Vec::new();
    let source_file = rule_ctx.source_file;
    for diagnostic in
        rule_ctx.checker().get_diagnostics_exported(&GoContext::background(), source_file)
    {
        out.push(format!("{}:{}", diagnostic.code(), diagnostic.localize()));
    }
    for diagnostic in rule_ctx.checker().get_global_diagnostics() {
        out.push(format!("global:{}:{}", diagnostic.code(), diagnostic.localize()));
    }
    out
}

// go test: internal/rules/api_stability_leak_regression_test.go:104 assertApiStabilitySourcesParse 6c45db01dca6d2de
// Rust: Go's assertion to `*compiler.Program` always succeeds (the helper builds a compiler program).
pub(crate) fn assert_api_stability_sources_parse(t: &T, files: &Files) {
    t.helper();
    let compiled = compile_api_stability_files(t, files);
    for diagnostic in compiled.program.get_syntactic_diagnostics(&GoContext::background(), None) {
        t.error(format!("syntactic diagnostic {}: {}", diagnostic.code(), diagnostic.localize()));
    }
}

// go test: internal/rules/api_stability_leak_regression_test.go:117 apiStabilityRegressionCase 68261a0f18f59638
pub(crate) struct ApiStabilityRegressionCase {
    pub(crate) name: String,
    pub(crate) files: Files,
    pub(crate) want: Vec<String>,
    pub(crate) not_want: Vec<String>,
}

/// Rust-only: Go's `apiStabilityRegressionCase{name, files, want, notWant}` literal (absent lists are empty).
pub(crate) fn case(
    name: &str,
    files: Files,
    want: &[&str],
    not_want: &[&str],
) -> ApiStabilityRegressionCase {
    ApiStabilityRegressionCase {
        name: name.to_string(),
        files,
        want: want.iter().map(|s| (*s).to_string()).collect(),
        not_want: not_want.iter().map(|s| (*s).to_string()).collect(),
    }
}

// go test: internal/rules/api_stability_leak_regression_test.go:124 apiStabilityMaxInstantiationDelta 766a9c4370342608
pub(crate) const API_STABILITY_MAX_INSTANTIATION_DELTA: u32 = 100_000;

// go test: internal/rules/api_stability_leak_regression_test.go:126 apiStabilityControlCompilerDiagnostics a5c47da7af76d24a
pub(crate) fn api_stability_control_compiler_diagnostics(t: &T, files: &Files) -> Vec<String> {
    t.helper();
    let mut compiled = compile_api_stability_files_primed(t, files, false);
    compiled.with_rule_ctx(api_stability_compiler_diagnostics)
}

// go test: internal/rules/api_stability_leak_regression_test.go:133 runApiStabilityRegressionCase a0d6c2e1c7878a3f
pub(crate) fn run_api_stability_regression_case(t: &T, test: &ApiStabilityRegressionCase) {
    t.helper();
    for cold in [false, true] {
        let mut name = test.name.clone();
        if cold {
            name += "/cold";
        } else {
            name += "/warm";
        }
        t.run(&name, |t| {
            let mut compiled = compile_api_stability_files_primed(t, &test.files, !cold);
            let (messages, delta, before_compiler, after_compiler) =
                compiled.with_rule_ctx(|rule_ctx| {
                    let mut before_compiler = Vec::new();
                    if !cold {
                        before_compiler = api_stability_compiler_diagnostics(rule_ctx);
                    }
                    let before_instantiation = rule_ctx.checker().total_instantiation_count;
                    let messages = run_api_stability_messages(t, rule_ctx);
                    let delta = rule_ctx.checker().total_instantiation_count - before_instantiation;
                    let after_compiler = api_stability_compiler_diagnostics(rule_ctx);
                    (messages, delta, before_compiler, after_compiler)
                });
            drop(compiled);

            if delta > API_STABILITY_MAX_INSTANTIATION_DELTA {
                t.error(format!(
                    "rule instantiated {delta} types, want at most {API_STABILITY_MAX_INSTANTIATION_DELTA}"
                ));
            }
            if cold {
                let control = api_stability_control_compiler_diagnostics(t, &test.files);
                if control != after_compiler {
                    t.error(format!(
                        "cold rule changed compiler diagnostics against control:\ncontrol={control:?}\nafter={after_compiler:?}"
                    ));
                }
            } else if before_compiler != after_compiler {
                t.error(format!(
                    "rule changed compiler diagnostics:\nbefore={before_compiler:?}\nafter={after_compiler:?}"
                ));
            }
            if !cold {
                for want in &test.want {
                    if !contains_api_stability_message(t, &messages, want) {
                        t.error(format!("missing diagnostic {want:?} in {messages:?}"));
                    }
                }
            }
            for not_want in &test.not_want {
                if contains_api_stability_message(t, &messages, not_want) {
                    t.error(format!("unexpected diagnostic {not_want:?} in {messages:?}"));
                }
            }
        });
    }
}

/// Rust-only: the loop every table test ends with (`for _, test := range tests { t.Run(test.name, ...) }`).
pub(crate) fn run_api_stability_regression_cases(t: &T, tests: &[ApiStabilityRegressionCase]) {
    for test in tests {
        t.run(&test.name, |t| {
            t.parallel();
            assert_api_stability_sources_parse(t, &test.files);
            run_api_stability_regression_case(t, test);
        });
    }
}

// go test: internal/rules/api_stability_leak_regression_test.go:180 apiStabilityExperimentalPrefix af87b235e7ebcf6c
pub(crate) const API_STABILITY_EXPERIMENTAL_PREFIX: &str =
    "/** @stability experimental */\ninterface E { value: string }\ndeclare const e: E\n";

// go test: internal/rules/api_stability_leak_regression_test.go:182 singleApiStabilityFile 127546e23e462399
pub(crate) fn single_api_stability_file(source: &str) -> Files {
    files(&[("test.ts", &cat(&[API_STABILITY_EXPERIMENTAL_PREFIX, source]))])
}

// go test: internal/rules/api_stability_leak_regression_test.go:186 apiStabilityBothOrders d4c2cfdb1a41e2d9
pub(crate) fn api_stability_both_orders(
    name: &str,
    prefix: &str,
    clean: &str,
    leaky: &str,
    not_want: &str,
) -> Vec<ApiStabilityRegressionCase> {
    vec![
        case(
            &format!("{name} clean first"),
            files(&[("test.ts", &cat(&[prefix, clean, leaky]))]),
            &["`leaky` exposes `E`"],
            &[not_want],
        ),
        case(
            &format!("{name} leaky first"),
            files(&[("test.ts", &cat(&[prefix, leaky, clean]))]),
            &["`leaky` exposes `E`"],
            &[not_want],
        ),
    ]
}

// go test: internal/rules/api_stability_leak_regression_test.go:203 TestApiStabilityLeakColdSafety 1e5911b4736931ed
#[test]
fn test_api_stability_leak_cold_safety() {
    run(|t| {
        t.parallel();

        let experimental_e = r"/** @stability experimental */
interface E { value: string }
";
        let deep_alias = "type Deep<T> = T extends string ? Deep<T> : T\n";
        let step_alias = "type Step<T extends unknown[]> = T extends [unknown, ...infer Tail] ? Step<Tail> : number\n";
        let test_file = |parts: &[&str]| files(&[("test.ts", &cat(parts))]);

        let mut tests = vec![
            case(
                "recursive member budget",
                test_file(&[
                    deep_alias,
                    r"interface Wrapper<T> { value: Deep<T> }
export declare const exposed: Wrapper<string>
",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "inferred getter budget",
                test_file(&[
                    deep_alias,
                    r"interface Callable<T> { (x: Deep<T>): void }
declare const budgetValue: Callable<string>
export class Owner { get method() { return budgetValue } }
",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "inferred getter return budget",
                test_file(&[
                    deep_alias,
                    r"interface Callable<T> { (): Deep<T> }
declare const budgetValue: Callable<string>
export class Owner { get method() { return budgetValue } }
",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "referenced non-generic getter budget",
                test_file(&[
                    deep_alias,
                    r"interface Callable<T> { (x: Deep<T>): void }
declare const budgetValue: Callable<string>
class Hidden { get method() { return budgetValue } }
export declare const owner: Hidden
",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "instantiated anonymous object budget",
                test_file(&[
                    deep_alias,
                    r"declare function make<T>(): { value: Deep<T> }
export const object = make<string>()
",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "inherited generic member budget",
                test_file(&[
                    deep_alias,
                    r"interface Base<T> { value: Deep<T> }
export interface Child extends Base<string> {}
",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "inherited generic signature budget",
                test_file(&[
                    deep_alias,
                    r"interface Base<T> { (x: T): void }
interface Child<T> extends Base<Deep<T>> {}
declare function make<T>(): Child<T>
export const f = make<string>()
",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "declared reference recursive argument budget",
                test_file(&[
                    deep_alias,
                    "interface Callable<T> { (x: T): void }\nexport declare const f: Callable<Deep<string>>",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "branch recursive concrete alias budget",
                test_file(&[
                    deep_alias,
                    "export type C<T> = T extends number ? Deep<string> : boolean",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "recursive named callable argument budget",
                test_file(&[
                    deep_alias,
                    "interface Callable<T> { (x: T): void }\ndeclare function make<T>(): Callable<Deep<T>>\nexport const f = make<string>()",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "annotated recursive parameter",
                test_file(&[deep_alias, "export declare function f(x: Deep<string>): void"]),
                &[],
                &["exposes"],
            ),
            case(
                "annotated recursive return",
                test_file(&[deep_alias, "export declare function f(): Deep<string>"]),
                &[],
                &["exposes"],
            ),
            case(
                "annotated recursive declared property",
                test_file(&[deep_alias, "export interface I { value: Deep<string> }"]),
                &[],
                &["exposes"],
            ),
            case(
                "annotated recursive inherited method",
                test_file(&[
                    deep_alias,
                    "interface Base { f(x: Deep<string>): void }\nexport interface Child extends Base {}",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "indexed recursive parameter",
                test_file(&[
                    deep_alias,
                    "interface I { x: Deep<string> }\nexport declare function f(x: I[\"x\"]): void",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "anonymous recursive index",
                test_file(&[
                    deep_alias,
                    "export declare function f(): { [key: string]: Deep<string> }",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "recursive generic default parameter",
                test_file(&[
                    deep_alias,
                    "interface I<T = Deep<string>> { x: T }\nexport declare function f(x: I): void",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "recursive generic default return",
                test_file(&[
                    deep_alias,
                    "interface I<T = Deep<string>> { x: T }\nexport declare function f(): I",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "cold typeof recursive callable",
                test_file(&[
                    deep_alias,
                    "declare function fn(x: Deep<string>): void\nexport declare const o: typeof fn",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "cold tagged anonymous recursive member",
                test_file(&[
                    deep_alias,
                    r"export declare const o: {
/** @stability experimental */
value: Deep<string>
}",
                ]),
                &["exposes"],
                &[],
            ),
            case(
                "finite recursive tuple application",
                test_file(&[
                    step_alias,
                    "export declare function f(x: Step<[",
                    &"0,".repeat(60),
                    "]>): void",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "replaced generic default is not reported",
                test_file(&[
                    experimental_e,
                    "interface Callable<T = E> { (x: T): T }\nexport declare const f: Callable<string>\n",
                ]),
                &[],
                &["`f` exposes `E`"],
            ),
            case(
                "replaced generic default instantiation expression",
                test_file(&[
                    experimental_e,
                    "declare function identity<T = E>(x: T): T\nexport const f = identity<string>\n",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "eliminated conditional branch",
                test_file(&[
                    experimental_e,
                    "export type Reduced = string extends string ? number : E\n",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "nested named surface stays shallow",
                test_file(&[
                    experimental_e,
                    "interface Hidden { value: E }\nexport declare const exposed: { hidden: Hidden }\n",
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "interface instantiated conditional eliminates branch",
                single_api_stability_file(
                    r"interface Holder<T> { value: T extends string ? number : E }
export declare const clean: Holder<string>
export declare const leaky: Holder<E>",
                ),
                &["`leaky` exposes `E`"],
                &["`clean` exposes `E`"],
            ),
            case(
                "alias instantiated conditional eliminates branch",
                single_api_stability_file(
                    r"type Holder<T> = { value: T extends string ? number : E }
type Direct<T> = T extends string ? number : E
export declare const clean: Holder<string>
export declare const leaky: Holder<E>
export declare const directClean: Direct<string>
export declare const directLeaky: Direct<E>",
                ),
                &["`leaky` exposes `E`", "`directLeaky` exposes `E`"],
                &["`clean` exposes `E`", "`directClean` exposes `E`"],
            ),
            case(
                "eliminated conditional alias application",
                single_api_stability_file(
                    "type Hidden<T> = T extends string ? E : never\nexport declare const clean: Hidden<E>",
                ),
                &[],
                &["exposes"],
            ),
            case(
                "compound argument conditional eliminated",
                single_api_stability_file(
                    "interface Holder<T> { value: T extends string[] ? number : E }\nexport declare const clean: Holder<string[]>",
                ),
                &[],
                &["exposes"],
            ),
            case(
                "never distributive conditional eliminated",
                single_api_stability_file(
                    "interface Holder<T> { value: T extends string ? E : number }\nexport declare const clean: Holder<never>",
                ),
                &[],
                &["exposes"],
            ),
        ];
        tests.extend(api_stability_both_orders(
            "instantiated signature contexts",
            &cat(&[experimental_e, "declare function make<T>(): (x: T) => T\n"]),
            "export const clean = make<string>()\n",
            "export const leaky = make<E>()\n",
            "`clean` exposes `E`",
        ));
        tests.extend(api_stability_both_orders(
            "nested instantiated contexts",
            &cat(&[
                experimental_e,
                "declare function make<T>(): { nested: { fn: (x: T) => T } }\n",
            ]),
            "export const clean = make<string>()\n",
            "export const leaky = make<E>()\n",
            "`clean` exposes `E`",
        ));
        tests.extend(api_stability_both_orders(
            "shared raw target",
            &cat(&[experimental_e, deep_alias, "interface DeepHolder<T> { value: Deep<T> }\n"]),
            "export declare const clean: DeepHolder<string>\n",
            "export declare const leaky: DeepHolder<E>\n",
            "`clean` exposes `E`",
        ));
        tests.extend(api_stability_both_orders(
            "shared heritage materialization",
            &cat(&[experimental_e, "interface Base<T> { value: T }\n"]),
            "export interface clean extends Base<string> {}\n",
            "export interface leaky extends Base<E> {}\n",
            "`clean` exposes `E`",
        ));

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:414 TestApiStabilityLeakDoesNotTruncateMembers 03656ab02d864305
#[test]
fn test_api_stability_leak_does_not_truncate_members() {
    run(|t| {
        t.parallel();

        let mut source = String::new();
        source.push_str("/** @stability experimental */\ninterface E { value: string }\n");
        source.push_str("export interface ManyMembers {\n");
        for index in 0..129 {
            writeln!(source, "  harmless{index}: string").expect("writing to a String");
        }
        source.push_str("  leaked: E\n}\n");

        let mut compiled = compile_api_stability_files(t, &files(&[("test.ts", &source)]));
        compiled.with_rule_ctx(|rule_ctx| {
            let messages = run_api_stability_messages(t, rule_ctx);
            if !contains_api_stability_message(t, &messages, "`ManyMembers` exposes `E`") {
                t.fatal(format!("leak after many members was not reported: {messages:?}"));
            }
        });
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:434 TestApiStabilityLeakInstantiationDoesNotScaleWithDepth 34c8dd9b22a615df
#[test]
fn test_api_stability_leak_instantiation_does_not_scale_with_depth() {
    run(|t| {
        t.parallel();

        let mut deltas = rustc_hash::FxHashMap::<i64, u32>::default();
        for depth in [2_i64, 6, 12, 24] {
            let repeat = usize::try_from(depth).expect("a small depth");
            let source = cat(&[
                "/** @stability experimental */\ninterface E { value: string }\ninterface Box<T> { value: T }\n",
                "export declare const deep: ",
                &"Box<".repeat(repeat),
                "E",
                &">".repeat(repeat),
                "\n",
            ]);

            let mut compiled =
                compile_api_stability_files_primed(t, &files(&[("test.ts", &source)]), true);
            let (messages, delta) = compiled.with_rule_ctx(|rule_ctx| {
                let before = rule_ctx.checker().total_instantiation_count;
                let messages = run_api_stability_messages(t, rule_ctx);
                let delta = rule_ctx.checker().total_instantiation_count - before;
                (messages, delta)
            });
            drop(compiled);

            if !contains_api_stability_message(t, &messages, "`deep` exposes `E`") {
                t.fatal(format!("depth {depth} lost its leak diagnostic: {messages:?}"));
            }
            if delta > API_STABILITY_MAX_INSTANTIATION_DELTA {
                t.error(format!(
                    "depth {depth} instantiated {delta} types, want at most {API_STABILITY_MAX_INSTANTIATION_DELTA}"
                ));
            }
            deltas.insert(depth, delta);
        }
        if deltas[&24] > deltas[&2] + 2 {
            t.error(format!(
                "instantiation delta scaled with depth: depth2={} depth24={}",
                deltas[&2], deltas[&24]
            ));
        }
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:461 TestApiStabilityLeakCyclicCompletionIsReusable 3735ffcf5895603c
#[test]
#[expect(clippy::literal_string_with_formatting_args, reason = "TypeScript fixtures with braces")]
fn test_api_stability_leak_cyclic_completion_is_reusable() {
    run(|t| {
        t.parallel();

        let orders = [
            "export type A={b:B;payload:E}\nexport type B={a:A}\n",
            "export type B={a:A}\nexport type A={b:B;payload:E}\n",
        ];
        for (index, body) in orders.into_iter().enumerate() {
            t.run(&format!("order={index}"), |t| {
                t.parallel();
                let files = files(&[(
                    "test.ts",
                    &cat(&[
                        r"/** @stability experimental */
interface E {value:string}
",
                        body,
                    ]),
                )]);
                for iteration in 0..10 {
                    let mut compiled = compile_api_stability_files_primed(t, &files, true);
                    let messages =
                        compiled.with_rule_ctx(|rule_ctx| run_api_stability_messages(t, rule_ctx));
                    drop(compiled);
                    if !contains_api_stability_message(t, &messages, "`A` exposes `E`")
                        || !contains_api_stability_message(t, &messages, "`B` exposes `E`")
                    {
                        t.fatal(format!(
                            "iteration={iteration} lost a cyclic diagnostic: {messages:?}"
                        ));
                    }
                }
            });
        }
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:487 TestApiStabilityNonInstantiatingAccessors 83ca3e9b50d2dbb3
#[test]
#[expect(clippy::literal_string_with_formatting_args, reason = "TypeScript fixtures with braces")]
fn test_api_stability_non_instantiating_accessors() {
    run(|t| {
        t.parallel();

        for branch in ["Box<string>", "Select<string>", "Deep<string>"] {
            t.run(&format!("branch/{branch}"), |t| {
                t.parallel();
                let source = cat(&[
                    r"interface Box<T>{value:T}
type Select<T> = T extends string ? {value:T} : T
type Deep<T> = T extends string ? Deep<T> : T
export type C<T> = T extends number ? ",
                    branch,
                    " : boolean",
                ]);
                let mut compiled =
                    compile_api_stability_files_primed(t, &files(&[("test.ts", &source)]), false);
                compiled.with_rule_ctx(|rule_ctx| {
                    let exports = api_stability_exports_of_source_file(rule_ctx);
                    if exports.len() != 1 {
                        t.fatal(format!("got {} exports, want 1", exports.len()));
                    }
                    let conditional = rule_ctx.checker().get_declared_type_of_symbol(exports[0]);
                    if !conditional.flags().intersects(TypeFlags::Conditional) {
                        t.fatal("declared type is not a conditional");
                    }
                    let before = rule_ctx.checker().total_instantiation_count;
                    if get_resolved_conditional_type_branch(Some(conditional), true).is_some() {
                        t.fatal("cold conditional branch should not be resolved");
                    }
                    let _ = get_conditional_type_branch_type(rule_ctx, conditional, true);
                    let delta = rule_ctx.checker().total_instantiation_count - before;
                    if delta != 0 {
                        t.error(format!("branch accessor instantiated {delta} types, want 0"));
                    }
                    let globals = rule_ctx.checker().get_global_diagnostics();
                    if !globals.is_empty() {
                        t.error(format!(
                            "branch accessor added global diagnostics: {:?}",
                            diagnostic_messages(&globals)
                        ));
                    }
                });
            });
        }

        t.run("index infos", |t| {
            t.parallel();
            let source = r"type Deep<T> = T extends string ? Deep<T> : T
export interface I<T> {[key:string]:Deep<string>}";
            let mut compiled =
                compile_api_stability_files_primed(t, &files(&[("test.ts", source)]), false);
            compiled.with_rule_ctx(|rule_ctx| {
                let exports = api_stability_exports_of_source_file(rule_ctx);
                if exports.len() != 1 {
                    t.fatal(format!("got {} exports, want 1", exports.len()));
                }
                let before = rule_ctx.checker().total_instantiation_count;
                let _ = rule_ctx.checker().get_members_of_symbol(exports[0]);
                let infos =
                    get_declared_index_infos_of_symbol(rule_ctx.checker(), Some(exports[0]));
                let delta = rule_ctx.checker().total_instantiation_count - before;
                if delta != 0 {
                    t.error(format!("index accessor instantiated {delta} types, want 0"));
                }
                for info in &infos {
                    if info.value_type().is_some() {
                        t.error("unresolved index value type should stay nil");
                    }
                }
                let globals = rule_ctx.checker().get_global_diagnostics();
                if !globals.is_empty() {
                    t.error(format!(
                        "index accessor added global diagnostics: {:?}",
                        diagnostic_messages(&globals)
                    ));
                }
            });
        });

        t.run("symbol and alias types", |t| {
            t.parallel();
            let source = r"type Deep<T> = T extends string ? Deep<T> : T
type S = Deep<string>
export declare const x: S";
            let mut compiled =
                compile_api_stability_files_primed(t, &files(&[("test.ts", source)]), false);
            compiled.with_rule_ctx(|rule_ctx| {
                let exports = api_stability_exports_of_source_file(rule_ctx);
                if exports.len() != 1 {
                    t.fatal(format!("got {} exports, want 1", exports.len()));
                }
                let before = rule_ctx.checker().total_instantiation_count;
                if get_resolved_type_of_symbol_if_materialized(rule_ctx.checker(), Some(exports[0]))
                    .is_some()
                {
                    t.fatal("cold export value type should not be materialized");
                }
                let _ = get_resolved_declared_type_of_symbol_if_materialized(
                    rule_ctx.checker(),
                    Some(exports[0]),
                );
                let delta = rule_ctx.checker().total_instantiation_count - before;
                if delta != 0 {
                    t.error(format!("symbol accessor instantiated {delta} types, want 0"));
                }
                let globals = rule_ctx.checker().get_global_diagnostics();
                if !globals.is_empty() {
                    t.error(format!(
                        "symbol accessor added global diagnostics: {:?}",
                        diagnostic_messages(&globals)
                    ));
                }
            });
        });
    });
}

/// Rust-only: Go's `ruleCtx.Checker.GetExportsOfModule(ruleCtx.TypeParser.GetSymbolAtLocation(sf.AsNode()))`.
fn api_stability_exports_of_source_file(
    rule_ctx: &mut Context<'_, '_>,
) -> Vec<P<ets_shim::ast::Symbol>> {
    let source_file = rule_ctx.source_file;
    let Some(module) = rule_ctx.type_parser.get_symbol_at_location(source_file.as_node()) else {
        // Go passes the nil symbol on, and the checker panics; no fixture gets here.
        panic!("no module symbol for {}", source_file.file_name());
    };
    rule_ctx.checker().get_exports_of_module_exported(module)
}

/// Rust-only: the shim's `checker.GetConditionalTypeBranchType(c, t, trueBranch)` (integration.go:
/// `GetResolvedTypeFromTypeNode(c, GetConditionalTypeBranchNode(c, t, trueBranch))`).
fn get_conditional_type_branch_type(
    rule_ctx: &mut Context<'_, '_>,
    t: P<ets_shim::checker::Type>,
    true_branch: bool,
) -> Option<P<ets_shim::checker::Type>> {
    let node = get_conditional_type_branch_node(Some(t), true_branch)?;
    get_resolved_type_from_type_node(rule_ctx.checker(), node)
}

/// Rust-only: Go's `%v` of diagnostics in failure messages.
fn diagnostic_messages(diagnostics: &[P<Diagnostic>]) -> Vec<String> {
    diagnostics.iter().map(|diagnostic| diagnostic.localize()).collect()
}

// go test: internal/rules/api_stability_leak_regression_test.go:575 TestApiStabilityLeakColdLazyReadMatchesWarm 6c2b4f39722e3c74
#[test]
fn test_api_stability_leak_cold_lazy_read_matches_warm() {
    run(|t| {
        t.parallel();

        let files = files(&[(
            "test.ts",
            r"/** @stability experimental */
interface E { value: string }
type Deep<T> = T extends string ? Deep<T> : T
interface Callable<T> { (x: T): void }
export declare const f: Callable<Deep<E>>
",
        )]);
        let mut compiled = compile_api_stability_files_primed(t, &files, false);
        compiled.with_rule_ctx(|rule_ctx| {
            let exports = api_stability_exports_of_source_file(rule_ctx);
            if exports.len() != 1 {
                t.fatal(format!("got {} exports, want 1", exports.len()));
            }
            let symbol = exports[0];

            let cold = rule_ctx.type_parser.api_stability_used_by_symbol(Some(symbol));
            if !contains_dependency_name(&cold, "E") {
                t.fatal(format!(
                    "the cold surface should lazily resolve E, got {:?}",
                    dependency_names(&cold)
                ));
            }

            let _ = api_stability_compiler_diagnostics(rule_ctx);
            let warm = rule_ctx.type_parser.api_stability_used_by_symbol(Some(symbol));
            if !contains_dependency_name(&warm, "E") {
                t.fatal(format!("warm surface should expose E, got {:?}", dependency_names(&warm)));
            }
            if cold.minimum() != warm.minimum()
                || cold.dependencies().len() != warm.dependencies().len()
            {
                t.fatal(format!(
                    "cold and warm surfaces disagree: cold={:?} warm={:?}",
                    dependency_names(&cold),
                    dependency_names(&warm)
                ));
            }
        });
    });
}

/// Rust-only: Go's `%v` of a surface's dependencies in failure messages (their names).
fn dependency_names(used: &ApiStabilityUsed) -> Vec<String> {
    used.dependencies()
        .all()
        .map(|(_, dependency)| api_stability_dependency_name(dependency))
        .collect()
}

// go test: internal/rules/api_stability_leak_regression_test.go:608 containsDependencyName d87c41945ee3aa66
fn contains_dependency_name(used: &ApiStabilityUsed, name: &str) -> bool {
    for (_, dependency) in used.dependencies().all() {
        if api_stability_dependency_name(dependency) == name {
            return true;
        }
    }
    false
}

// go test: internal/rules/api_stability_leak_regression_test.go:617 apiStabilityColdUnavailableProbes c05ee930ca7e1a0c
fn api_stability_cold_unavailable_probes() -> Vec<(&'static str, String)> {
    let deep_alias = "type Deep<T> = T extends string ? Deep<T> : T\n";
    vec![
        (
            "indexed recursive parameter",
            cat(&[
                deep_alias,
                r#"interface I { x: Deep<string> }
export declare function f(x: I["x"]): void"#,
            ]),
        ),
        (
            "keyof recursive mapped",
            cat(&[
                deep_alias,
                r"interface I extends Deep<string> {}
export declare function f(x: keyof I): void",
            ]),
        ),
        (
            "anonymous recursive index",
            cat(&[deep_alias, r"export declare function f(): { [key: string]: Deep<string> }"]),
        ),
        (
            "recursive generic default parameter",
            cat(&[
                deep_alias,
                r"interface I<T = Deep<string>> { x: T }
export declare function f(x: I): void",
            ]),
        ),
        (
            "recursive generic default return",
            cat(&[
                deep_alias,
                r"interface I<T = Deep<string>> { x: T }
export declare function f(): I",
            ]),
        ),
        (
            "inferred getter",
            cat(&[
                deep_alias,
                r"declare const deep: Deep<string>
export class C { get x() { return deep } }",
            ]),
        ),
        (
            "inferred parameter initializer",
            cat(&[
                deep_alias,
                r"declare const deep: Deep<string>
export function f(x = deep): void {}",
            ]),
        ),
        (
            "inferred return",
            cat(&[
                deep_alias,
                r"declare const deep: Deep<string>
export function f() { return deep }",
            ]),
        ),
        (
            "anonymous nested inferred",
            cat(&[
                deep_alias,
                r"declare const deep: Deep<string>
export const x = { nested: { deep } }",
            ]),
        ),
        ("mapped template", cat(&[deep_alias, r#"export type M = { [K in "x"]: Deep<string> }"#])),
        (
            "mapped name",
            cat(&[deep_alias, r#"export type M = { [K in "x" as Deep<string>]: number }"#]),
        ),
        (
            "type literal alias index",
            cat(&[deep_alias, r"export type M = { [x: string]: Deep<string> }"]),
        ),
    ]
}

// go test: internal/rules/api_stability_leak_regression_test.go:649 TestApiStabilityLeakColdUnavailableIsSafe ea596ff90ddbd7d1
#[test]
fn test_api_stability_leak_cold_unavailable_is_safe() {
    run(|t| {
        t.parallel();

        for (name, source) in api_stability_cold_unavailable_probes() {
            t.run(name, |t| {
                t.parallel();
                let files = files(&[("test.ts", &source)]);
                let mut compiled = compile_api_stability_files_primed(t, &files, false);
                let (messages, delta, compiler, globals) = compiled.with_rule_ctx(|rule_ctx| {
                    let before = rule_ctx.checker().total_instantiation_count;
                    let messages = run_api_stability_messages(t, rule_ctx);
                    let delta = rule_ctx.checker().total_instantiation_count - before;
                    let compiler = api_stability_compiler_diagnostics(rule_ctx);
                    let globals = rule_ctx.checker().get_global_diagnostics().len();
                    (messages, delta, compiler, globals)
                });
                drop(compiled);

                if delta > API_STABILITY_MAX_INSTANTIATION_DELTA {
                    t.error(format!(
                        "cold rule instantiated {delta} types, want at most {API_STABILITY_MAX_INSTANTIATION_DELTA}"
                    ));
                }
                if globals != 0 {
                    t.error(format!("cold rule produced {globals} global diagnostics"));
                }
                if !messages.is_empty() {
                    t.error(format!("cold rule fabricated diagnostics: {messages:?}"));
                }
                let control = api_stability_control_compiler_diagnostics(t, &files);
                if control != compiler {
                    t.error(format!(
                        "cold rule changed compiler diagnostics against control:\ncontrol={control:?}\nafter={compiler:?}"
                    ));
                }
            });
        }
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:680 TestApiStabilityLeakRepresentedConditionalOutcomes 98f7e6c1e7721437
#[test]
fn test_api_stability_leak_represented_conditional_outcomes() {
    run(|t| {
        t.parallel();

        let prefix = "/** @stability experimental */\ninterface E { value: string }\n";
        // Rust: Go's anonymous `struct{name, source, leak}` cases.
        let cases = [
            (
                "contravariant conditional",
                r#"interface C<in T> { f: (x: T) => void }
interface Holder<T> { value: T extends C<string> ? number : E }
export declare const x: Holder<C<"x">>"#,
                true,
            ),
            (
                "invariant conditional",
                r#"interface C<in out T> { f: (x: T) => T }
interface Holder<T> { value: T extends C<string> ? number : E }
export declare const x: Holder<C<"x">>"#,
                true,
            ),
            (
                "array wrapped conditional",
                r#"interface C<in T> { f: (x: T) => void }
interface Holder<T> { value: T extends C<string>[] ? number : E }
export declare const x: Holder<C<"x">[]>"#,
                true,
            ),
            (
                "structural conditional",
                r"interface Holder<T> { value: T extends { value: string } ? number : E }
export declare const x: Holder<{ value: string }>",
                false,
            ),
        ];

        for (name, source, leak) in cases {
            t.run(name, |t| {
                t.parallel();
                let mut compiled = compile_api_stability_files_primed(
                    t,
                    &files(&[("test.ts", &cat(&[prefix, source]))]),
                    true,
                );
                compiled.with_rule_ctx(|rule_ctx| {
                    // Fully represent the conditional outcome: ask the checker for the
                    // instantiated member type, which materializes the selected branch.
                    for symbol in api_stability_exports_of_source_file(rule_ctx) {
                        if symbol.name() != "x" {
                            continue;
                        }
                        let value = rule_ctx.checker().get_type_of_symbol(symbol);
                        for property in rule_ctx.checker().get_properties_of_type(value) {
                            if property.name() == "value" {
                                let _ = rule_ctx.checker().get_type_of_symbol(*property);
                            }
                        }
                    }

                    let messages = run_api_stability_messages(t, rule_ctx);
                    if leak {
                        if !contains_api_stability_message(t, &messages, "`x` exposes `E`") {
                            t.error(format!("missing diagnostic in {messages:?}"));
                        }
                    } else if contains_api_stability_message(t, &messages, "`x` exposes `E`") {
                        t.error(format!("unexpected diagnostic in {messages:?}"));
                    }
                });
            });
        }
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:748 TestApiStabilityLeakLazyConditionalOutcomes a6b60d6dd0f7caca
#[test]
fn test_api_stability_leak_lazy_conditional_outcomes() {
    run(|t| {
        t.parallel();

        let prefix = "/** @stability experimental */\ninterface E { value: string }\n";
        // Rust: Go's anonymous `struct{name, source, leak}` cases.
        let cases = [
            (
                "contravariant conditional selected branch",
                r#"interface C<in T> { f: (x: T) => void }
interface Holder<T> { value: T extends C<string> ? number : E }
export declare const x: Holder<C<"x">>"#,
                true,
            ),
            (
                "structural conditional selected branch",
                r"interface Holder<T> { value: T extends { value: string } ? number : E }
export declare const x: Holder<{ value: string }>",
                false,
            ),
        ];

        for (case_name, source, leak) in cases {
            for cold in [false, true] {
                let mut name = case_name.to_string();
                if cold {
                    name += "/cold";
                } else {
                    name += "/warm";
                }
                t.run(&name, |t| {
                    t.parallel();
                    let files = files(&[("test.ts", &cat(&[prefix, source]))]);
                    let mut compiled = compile_api_stability_files_primed(t, &files, !cold);
                    let (messages, compiler, incomplete) = compiled.with_rule_ctx(|rule_ctx| {
                        let messages = run_api_stability_messages(t, rule_ctx);
                        let compiler = api_stability_compiler_diagnostics(rule_ctx);
                        let mut session = rule_ctx.type_parser.new_api_stability_session();
                        let mut incomplete = false;
                        for symbol in api_stability_exports_of_source_file(rule_ctx) {
                            let used = session.used_by_symbol(rule_ctx.type_parser, Some(symbol));
                            if used.incomplete() {
                                incomplete = true;
                            }
                        }
                        (messages, compiler, incomplete)
                    });
                    drop(compiled);
                    let got = contains_api_stability_message(t, &messages, "`x` exposes `E`");
                    if got != leak {
                        t.error(format!("leak={got} want={leak}: {messages:?}"));
                    }
                    if incomplete {
                        t.error("a resolved conditional outcome should be a complete surface");
                    }
                    if cold {
                        let control = api_stability_control_compiler_diagnostics(t, &files);
                        if control != compiler {
                            t.error(format!(
                                "cold rule changed compiler diagnostics against control:\ncontrol={control:?}\nafter={compiler:?}"
                            ));
                        }
                    }
                });
            }
        }
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:810 TestApiStabilityLeakDeclaredIndexSignatureTags 5f079fbd8f9eada9
#[test]
fn test_api_stability_leak_declared_index_signature_tags() {
    run(|t| {
        t.parallel();

        // Rust: Go's anonymous `struct{name, source}` cases.
        let cases = [
            (
                "declared interface index",
                r"export interface I {
  /** @stability experimental */
  [key: string]: number
}",
            ),
            (
                "inline type literal index",
                r"export declare const x: {
  /** @stability experimental */
  [key: string]: number
}",
            ),
        ];

        for (case_name, source) in cases {
            for cold in [false, true] {
                let mut name = case_name.to_string();
                if cold {
                    name += "/cold";
                } else {
                    name += "/warm";
                }
                t.run(&name, |t| {
                    t.parallel();
                    let mut compiled = compile_api_stability_files_primed(
                        t,
                        &files(&[("test.ts", source)]),
                        !cold,
                    );
                    compiled.with_rule_ctx(|rule_ctx| {
                        let messages = run_api_stability_messages(t, rule_ctx);
                        if !contains_api_stability_message(t, &messages, "exposes") {
                            t.fatal(format!(
                                "missing index signature tag diagnostic in {messages:?}"
                            ));
                        }
                    });
                });
            }
        }
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:854 TestApiStabilityLeakFiniteRecursionDoesNotInstantiate 55335091a4a2c624
#[test]
fn test_api_stability_leak_finite_recursion_does_not_instantiate() {
    run(|t| {
        t.parallel();

        let step = "type Step<T extends unknown[]> = T extends [unknown, ...infer Tail] ? Step<Tail> : number\n";
        let application = cat(&["declare const deep: Step<[", &"0,".repeat(60), "]>\n"]);
        // Rust: Go's anonymous `struct{name, source}` cases.
        let cases = [
            (
                "indexed parameter",
                cat(&[
                    step,
                    r"interface I { value: Step<[",
                    &"0,".repeat(60),
                    r#"]> }
export declare function f(x: I["value"]): void"#,
                ]),
            ),
            (
                "inferred getter",
                cat(&[step, &application, r"export class C { get x() { return deep } }"]),
            ),
            (
                "inferred parameter initializer",
                cat(&[step, &application, r"export function f(x = deep): void {}"]),
            ),
            ("inferred return", cat(&[step, &application, r"export function f() { return deep }"])),
            (
                "anonymous nested inferred",
                cat(&[step, &application, r"export const x = { nested: { deep } }"]),
            ),
        ];

        for (name, source) in cases {
            t.run(name, |t| {
                t.parallel();
                let files = files(&[("test.ts", &source)]);
                let mut compiled = compile_api_stability_files_primed(t, &files, false);
                let (messages, delta, compiler, globals) = compiled.with_rule_ctx(|rule_ctx| {
                    let before = rule_ctx.checker().total_instantiation_count;
                    let messages = run_api_stability_messages(t, rule_ctx);
                    let delta = rule_ctx.checker().total_instantiation_count - before;
                    let compiler = api_stability_compiler_diagnostics(rule_ctx);
                    let globals = rule_ctx.checker().get_global_diagnostics().len();
                    (messages, delta, compiler, globals)
                });
                drop(compiled);

                if delta > API_STABILITY_MAX_INSTANTIATION_DELTA {
                    t.error(format!(
                        "rule instantiated {delta} types, want at most {API_STABILITY_MAX_INSTANTIATION_DELTA}"
                    ));
                }
                if globals != 0 {
                    t.error(format!("rule produced {globals} global diagnostics"));
                }
                if !messages.is_empty() {
                    t.error(format!("rule produced diagnostics: {messages:?}"));
                }
                let control = api_stability_control_compiler_diagnostics(t, &files);
                if control != compiler {
                    t.error(format!(
                        "cold rule changed compiler diagnostics against control:\ncontrol={control:?}\nafter={compiler:?}"
                    ));
                }
            });
        }
    });
}
