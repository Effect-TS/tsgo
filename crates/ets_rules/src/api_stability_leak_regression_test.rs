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

// go test: internal/rules/api_stability_leak_regression_test.go:27 compileApiStabilityFiles 507367180ecf6a7d
// compileApiStabilityFiles compiles a multi-file virtual project and runs the
// rule against test.ts. It mirrors compileApiStabilitySource but supports the
// dependency files needed to exercise namespace, barrel and star re-exports.
pub(crate) fn compile_api_stability_files(t: &T, files: &Files) -> ApiStabilityCompiled {
    t.helper();
    compile_api_stability_files_primed(t, files, true)
}

// go test: internal/rules/api_stability_leak_regression_test.go:36 compileApiStabilityFilesPrimed 48d32b9e168ec4cb
// compileApiStabilityFilesPrimed optionally primes semantic diagnostics before
// handing back the checker. Primed checkers model the language service warm
// path; unprimed checkers model a cold first run where the rule itself may
// trigger checking. Both must be free of instantiation and added diagnostics.
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

// go test: internal/rules/api_stability_leak_regression_test.go:83 runApiStabilityMessages b7a9cacc63996f48
pub(crate) fn run_api_stability_messages(t: &T, rule_ctx: &mut Context<'_, '_>) -> Vec<String> {
    t.helper();
    let diagnostics = (API_STABILITY_LEAK.run)(rule_ctx);
    let mut messages = Vec::with_capacity(diagnostics.len());
    for diagnostic in &diagnostics {
        messages.push(diagnostic.localize());
    }
    messages
}

// go test: internal/rules/api_stability_leak_regression_test.go:93 containsApiStabilityMessage 623ef286624fa5b0
pub(crate) fn contains_api_stability_message(t: &T, messages: &[String], want: &str) -> bool {
    t.helper();
    messages.iter().any(|message| message.contains(want))
}

// go test: internal/rules/api_stability_leak_regression_test.go:103 apiStabilityCompilerDiagnostics 968fec381830827d
// apiStabilityCompilerDiagnostics snapshots the source-file and global
// diagnostics the checker currently reports, including codes such as TS2589
// ("Type instantiation is excessively deep").
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

// go test: internal/rules/api_stability_leak_regression_test.go:117 assertApiStabilitySourcesParse d670ccf08731586a
// assertApiStabilitySourcesParse guards the regression fixtures themselves:
// broken type syntax would silently disable the coverage they are meant to
// provide.
//
// Rust: Go's assertion to `*compiler.Program` always succeeds (the helper builds a compiler program).
pub(crate) fn assert_api_stability_sources_parse(t: &T, files: &Files) {
    t.helper();
    let compiled = compile_api_stability_files(t, files);
    for diagnostic in compiled.program.get_syntactic_diagnostics(&GoContext::background(), None) {
        t.error(format!("syntactic diagnostic {}: {}", diagnostic.code(), diagnostic.localize()));
    }
}

// go test: internal/rules/api_stability_leak_regression_test.go:130 apiStabilityRegressionCase 68261a0f18f59638
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

// go test: internal/rules/api_stability_leak_regression_test.go:143 apiStabilityMaxInstantiationDelta a0ff5a52321e07ea
// apiStabilityMaxInstantiationDelta bounds the checker instantiations the rule
// may trigger for one case. Ordinary public surfaces stay far below it; a
// recursive alias application reached through a lazy public read is refused by
// the pre-flight recursion guard before the checker touches it, so the work is
// bounded and must not scale with recursion depth. The non-scaling test
// TestApiStabilityLeakInstantiationDoesNotScaleWithDepth pins that property.
pub(crate) const API_STABILITY_MAX_INSTANTIATION_DELTA: u32 = 100_000;

// go test: internal/rules/api_stability_leak_regression_test.go:148 apiStabilityControlCompilerDiagnostics ad318c2dae2fc865
// apiStabilityControlCompilerDiagnostics compiles the same files without ever
// running the rule and snapshots the checker's diagnostics, so a genuinely cold
// path can be compared against a control that the rule never touched.
pub(crate) fn api_stability_control_compiler_diagnostics(t: &T, files: &Files) -> Vec<String> {
    t.helper();
    let mut compiled = compile_api_stability_files_primed(t, files, false);
    compiled.with_rule_ctx(api_stability_compiler_diagnostics)
}

// go test: internal/rules/api_stability_leak_regression_test.go:174 runApiStabilityRegressionCase caf6a8c5a9cf1595
// runApiStabilityRegressionCase runs a case against both a warm (primed) and a
// genuinely cold (unprimed) checker.
//
// The warm checker models the normal rule lifecycle: the file has been type
// checked, so the public types the surface needs are materialized and every
// expected diagnostic must be reported. The cold checker models a first run
// before any checking: type-driven components are not materialized, so the
// analysis must return an incomplete result that is never reported as stable
// instead of resolving syntax or triggering recursive instantiation. A cold run
// therefore must produce no false positive and must leave the compiler's
// diagnostics unchanged; expecting a cold diagnostic is only valid for
// metadata-driven surfaces (declared `@stability` tags), which this harness
// still accepts
// through notWant only. Cold results that can be completed from metadata alone
// may legitimately report extra true diagnostics.
//
// The cold path snapshots diagnostics only after the rule ran and compares them
// with a separate control program that never ran the rule, so a TS2589 the rule
// provokes cannot be masked by pre-warming the checker.
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

// go test: internal/rules/api_stability_leak_regression_test.go:221 apiStabilityExperimentalPrefix af87b235e7ebcf6c
pub(crate) const API_STABILITY_EXPERIMENTAL_PREFIX: &str =
    "/** @stability experimental */\ninterface E { value: string }\ndeclare const e: E\n";

// go test: internal/rules/api_stability_leak_regression_test.go:223 singleApiStabilityFile 127546e23e462399
pub(crate) fn single_api_stability_file(source: &str) -> Files {
    files(&[("test.ts", &cat(&[API_STABILITY_EXPERIMENTAL_PREFIX, source]))])
}

// go test: internal/rules/api_stability_leak_regression_test.go:230 TestApiStabilityLeakFindingRegressions 83e36ac2c78d959a
// TestApiStabilityLeakFindingRegressions pins the original findings:
// instantiation safety, inline surfaces, erased aliases, constraints/defaults,
// bounded traversal and accessibility.
#[test]
fn test_api_stability_leak_finding_regressions() {
    run(|t| {
        t.parallel();

        let deep_boxes = cat(&[&"Box<".repeat(6), "E", &">".repeat(6)]);
        let tests = vec![
            case(
                "generic budget is not exhausted",
                files(&[(
                    "test.ts",
                    r"type Deep<T> = T extends string ? Deep<T> : T
interface Wrapper<T> { value: Deep<T> }
export declare const exposed: Wrapper<string>
",
                )]),
                &[],
                &["exposes"],
            ),
            case(
                "inline public object surfaces",
                files(&[(
                    "test.ts",
                    r"/** @stability experimental */
interface E { value: string }
export declare function parameter(x: { value: E }): void
export declare function result(): { value: E }
export interface Nested { nested: { value: E } }
export type Combined = { value: E } & { other: string }
",
                )]),
                &[
                    "`parameter` exposes `E`",
                    "`result` exposes `E`",
                    "`Nested` exposes `E`",
                    "`Combined` exposes `E`",
                ],
                &[],
            ),
            case(
                "erased alias annotation",
                files(&[(
                    "test.ts",
                    r"/** @stability experimental */
type ExperimentalText = string
export declare function consume(x: ExperimentalText): ExperimentalText
",
                )]),
                &["`consume` exposes `ExperimentalText`"],
                &[],
            ),
            case(
                "generic constraints and defaults",
                files(&[(
                    "test.ts",
                    r"/** @stability experimental */
interface E { value: string }
export declare function constrained<T extends E>(x: T): T
export declare function defaulted<T = E>(): T
export interface Generic<T extends E> { value: T }
export type Indexed<K extends keyof E> = E[K]
export type Conditional<T> = T extends E ? E : string
",
                )]),
                &[
                    "`constrained` exposes `E`",
                    "`defaulted` exposes `E`",
                    "`Generic` exposes `E`",
                    "`Indexed` exposes `E`",
                    "`Conditional` exposes `E`",
                ],
                &[],
            ),
            case(
                "finite traversal is not truncated",
                files(&[(
                    "test.ts",
                    &cat(&[
                        r"/** @stability experimental */
interface E { value: string }
interface Box<T> { value: T }
export declare const deep: ",
                        &deep_boxes,
                        r"
export interface OrderSensitive {
  deep: Box<Box<Box<Box<E>>>>
  direct: Box<E>
}
",
                    ]),
                )]),
                &["`deep` exposes `E`", "`OrderSensitive` exposes `E`"],
                &[],
            ),
            case(
                "public private classification",
                files(&[(
                    "test.ts",
                    r"/** @stability experimental */
interface E { value: string }
export interface Public { __public: E }
export class Hidden { #secret: E | undefined }
export class Factory { private constructor(x: E) {} }
",
                )]),
                &["`Public` exposes `E`"],
                &["`Hidden`", "`Factory`"],
            ),
            case(
                "callable surfaces",
                files(&[(
                    "test.ts",
                    r"/** @stability experimental */
interface E { value: string }
interface Callable { (x: E): void }
export interface Owner { value: Callable }
export interface Inline { value: (x: E) => void }
",
                )]),
                &["`Owner` exposes `E`", "`Inline` exposes `E`"],
                &[],
            ),
            case(
                "namespace reexport",
                files(&[
                    ("test.ts", "export * as api from \"./dep\"\n"),
                    (
                        "dep.ts",
                        r"/** @stability experimental */
export interface E { value: string }
export interface Leaky { value: E }
",
                    ),
                ]),
                &["`api` exposes `E`"],
                &[],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:348 TestApiStabilityLeakInferredAndReexportRegressions a103139b375f85ae
// TestApiStabilityLeakInferredAndReexportRegressions pins the inferred-surface
// findings: inferred accessor budget safety, inferred/arrow/callable
// signatures, inferred anonymous surfaces, inferred parameters, alias and
// namespace barrels, hidden overload implementations and reexport tags.
#[test]
fn test_api_stability_leak_inferred_and_reexport_regressions() {
    run(|t| {
        t.parallel();

        let generic_callable_budget = r"type Deep<T> = T extends string ? Deep<T> : T
interface Callable<T> { (x: Deep<T>): void }
declare const budgetValue: Callable<string>
export class Owner { get method() { return budgetValue } }
"
        .to_string();
        let generic_callable_return_budget = r"type Deep<T> = T extends string ? Deep<T> : T
interface Callable<T> { (): Deep<T> }
declare const budgetValue: Callable<string>
export class Owner { get method() { return budgetValue } }
"
        .to_string();
        let referenced_getter_budget = r"type Deep<T> = T extends string ? Deep<T> : T
interface Callable<T> { (x: Deep<T>): void }
declare const budgetValue: Callable<string>
class Hidden { get method() { return budgetValue } }
export declare const owner: Hidden
"
        .to_string();
        let barrel_dep = r"/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }
"
        .to_string();
        let tests = vec![
            // Finding 1: inferred accessors must not instantiate referenced generic signatures.
            case(
                "inferred getter budget",
                files(&[("test.ts", &generic_callable_budget)]),
                &[],
                &["exposes"],
            ),
            case(
                "inferred getter return budget",
                files(&[("test.ts", &generic_callable_return_budget)]),
                &[],
                &["exposes"],
            ),
            case(
                "referenced non-generic getter budget",
                files(&[("test.ts", &referenced_getter_budget)]),
                &[],
                &["exposes"],
            ),
            // Finding 2: exported callable values keep their public signatures.
            case(
                "arrow explicit",
                single_api_stability_file(r"export const arrow = (x: E): E => x"),
                &["`arrow` exposes `E`"],
                &[],
            ),
            case(
                "default arrow",
                single_api_stability_file(r"export default (x: E): E => x"),
                &["`default` exposes `E`"],
                &[],
            ),
            case(
                "function expression",
                single_api_stability_file(r"export const f = function (x: E): E { return x }"),
                &["`f` exposes `E`"],
                &[],
            ),
            case(
                "typeof generic function",
                single_api_stability_file(&cat(&[
                    r"declare function generic<T>(x: E): T",
                    "\n",
                    r"export type F = typeof generic",
                ])),
                &["`F` exposes `E`"],
                &[],
            ),
            case(
                "generic callable direct export",
                single_api_stability_file(
                    "interface Callable<T> { (x: E): T }\nexport declare const callable: Callable<string>",
                ),
                &["`callable` exposes `E`"],
                &[],
            ),
            case(
                "inferred callable reference",
                single_api_stability_file(
                    "interface Callable { (x: E): E }\ndeclare const callable: Callable\nexport const f = callable",
                ),
                &["`f` exposes `E`"],
                &[],
            ),
            // Finding 3: direct inferred object surfaces and accessor values.
            case(
                "inferred object",
                single_api_stability_file(r"export const object = { value: e }"),
                &["`object` exposes `E`"],
                &[],
            ),
            case(
                "inferred object nested",
                single_api_stability_file(r"export const object = { nested: { value: e } }"),
                &["`object` exposes `E`"],
                &[],
            ),
            case(
                "inferred function object",
                single_api_stability_file(r"export function result() { return { value: e } }"),
                &["`result` exposes `E`"],
                &[],
            ),
            case(
                "inferred function nested object",
                single_api_stability_file(
                    r"export function result() { return { nested: { value: e } } }",
                ),
                &["`result` exposes `E`"],
                &[],
            ),
            case(
                "inferred arrow nested object",
                single_api_stability_file(
                    r"export const result = () => ({ nested: { value: e } })",
                ),
                &["`result` exposes `E`"],
                &[],
            ),
            case(
                "inferred class property object",
                single_api_stability_file(r"export class C { property = { value: e } }"),
                &["`C` exposes `E`"],
                &[],
            ),
            case(
                "inferred getter",
                single_api_stability_file(r"export class C { get value() { return e } }"),
                &["`C` exposes `E`"],
                &[],
            ),
            case(
                "inferred class arrow property",
                single_api_stability_file(r"export class C { value = (x: E): E => x }"),
                &["`C` exposes `E`"],
                &[],
            ),
            case(
                "class expression",
                single_api_stability_file(
                    r"export const C = class { value: E = e; constructor(x: E) {} }",
                ),
                &["`C` exposes `E`"],
                &[],
            ),
            case(
                "typeof inferred object",
                single_api_stability_file(
                    "const inferred = { value: e }\nexport type F = typeof inferred",
                ),
                &["`F` exposes `E`"],
                &[],
            ),
            case(
                "typeof inferred arrow",
                single_api_stability_file(
                    "const inferred = (x: E): E => x\nexport type F = typeof inferred",
                ),
                &["`F` exposes `E`"],
                &[],
            ),
            // Finding 4: inferred parameters, including constructors.
            case(
                "parameter initializer",
                single_api_stability_file(r"export function consume(x = e): void {}"),
                &["`consume` exposes `E`"],
                &[],
            ),
            case(
                "constructor parameter initializer",
                single_api_stability_file(r"export class Factory { constructor(x = e) {} }"),
                &["`Factory` exposes `E`"],
                &[],
            ),
            case(
                "inferred parameter object",
                single_api_stability_file(r"export function consume(x = { value: e }): void {}"),
                &["`consume` exposes `E`"],
                &[],
            ),
            // Finding 5: alias surfaces and namespace barrels.
            case(
                "namespace barrel aliased member",
                files(&[
                    ("test.ts", r#"export * as api from "./barrel""#),
                    ("barrel.ts", r#"export { Leaky } from "./dep""#),
                    ("dep.ts", &barrel_dep),
                ]),
                &["`api` exposes `E`"],
                &[],
            ),
            case(
                "namespace nested reexport",
                files(&[
                    ("test.ts", r#"export * as api from "./barrel""#),
                    ("barrel.ts", r#"export * as nested from "./dep""#),
                    ("dep.ts", &barrel_dep),
                ]),
                &["`api` exposes `E`"],
                &[],
            ),
            case(
                "imported callable",
                files(&[
                    (
                        "test.ts",
                        r#"import type { Callable } from "./dep"
export interface O { f: Callable }
"#,
                    ),
                    (
                        "dep.ts",
                        r"/** @stability experimental */
interface E { value: string }
export interface Callable { (x: E): E }
",
                    ),
                ]),
                &["`O` exposes `E`"],
                &[],
            ),
            case(
                "imported alias argument",
                files(&[
                    (
                        "test.ts",
                        r#"import type { Holder } from "./dep"
export declare const x: Holder
"#,
                    ),
                    (
                        "dep.ts",
                        r"/** @stability experimental */
interface E { value: string }
interface Box<T> { value: T }
export type Holder = Box<E>
",
                    ),
                ]),
                &["`x` exposes `E`"],
                &[],
            ),
            case(
                "namespace cycle terminates",
                files(&[
                    ("test.ts", r#"export * as api from "./barrel""#),
                    (
                        "barrel.ts",
                        r#"export * as back from "./test"
/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }
"#,
                    ),
                ]),
                &["`api` exposes `E`"],
                &[],
            ),
            case(
                "namespace alias cycle terminates",
                files(&[
                    ("test.ts", r#"export { x } from "./dep""#),
                    ("dep.ts", r#"export { x } from "./test""#),
                ]),
                &[],
                &["exposes"],
            ),
            // Finding 6: hidden overload implementations do not leak.
            case(
                "overload implementation hidden",
                single_api_stability_file(
                    r#"export function f(x: string): string
export function f(x: string | E): string { return "" }
"#,
                ),
                &[],
                &["exposes"],
            ),
            case(
                "overload public signature leaks",
                single_api_stability_file(
                    r"export function f(x: E): E
export function f(x: string): string
export function f(x: E | string): E | string { return x as E }
",
                ),
                &["`f` exposes `E`"],
                &[],
            ),
            // Finding 7: tags on reexport declarations govern their ceiling.
            case(
                "named reexport experimental ceiling",
                files(&[
                    (
                        "test.ts",
                        r#"/** @stability experimental */
export { Leaky } from "./dep"
"#,
                    ),
                    ("dep.ts", &barrel_dep),
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "named reexport warns without tag",
                files(&[("test.ts", r#"export { Leaky } from "./dep""#), ("dep.ts", &barrel_dep)]),
                &["`Leaky` exposes `E`"],
                &[],
            ),
            case(
                "namespace unstable ceiling",
                files(&[
                    (
                        "test.ts",
                        r#"/** @stability unstable */
export * as api from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        r"/** @stability unstable */
export interface U { value: string }
export interface Leaky { value: U }
",
                    ),
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "namespace experimental ceiling",
                files(&[
                    (
                        "test.ts",
                        r#"/** @stability experimental */
export * as api from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        r"/** @stability experimental */
export interface E { value: string }
export interface Leaky { value: E }
",
                    ),
                ]),
                &[],
                &["exposes"],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:586 TestApiStabilityLeakInstantiationAndSignatureRegressions cb97d5af4177a335
// TestApiStabilityLeakInstantiationAndSignatureRegressions pins instantiated
// anonymous budget safety, signature substitutions,
// instantiation expressions, public generic callable surfaces inside members and
// unions, eliminated conditional branches, erased accessor aliases, namespace
// member stability and star re-export tags.
#[test]
fn test_api_stability_leak_instantiation_and_signature_regressions() {
    run(|t| {
        t.parallel();

        let experimental_e = r"/** @stability experimental */
interface E { value: string }
"
        .to_string();
        let tests = vec![
            // P1: an anonymous object instantiated from a generic function may
            // contain a lazily instantiated recursive member. Reading its members
            // through the uninstantiated target plus the mapper must not evaluate
            // `Deep<string>`.
            case(
                "instantiated anonymous object budget",
                files(&[(
                    "test.ts",
                    r"type Deep<T> = T extends string ? Deep<T> : T
declare function make<T>(): { value: Deep<T> }
export const object = make<string>()
",
                )]),
                &[],
                &["exposes"],
            ),
            case(
                "inferred generic return substitution",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"declare function make<T>(): (x: T) => T
export const f = make<E>()
",
                    ]),
                )]),
                &["`f` exposes `E`"],
                &[],
            ),
            case(
                "instantiation expression signature",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"declare function identity<T>(x: T): T
export const f = identity<E>
",
                    ]),
                )]),
                &["`f` exposes `E`"],
                &[],
            ),
            case(
                "replaced generic default is not reported",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"interface Callable<T = E> { (x: T): T }
export declare const f: Callable<string>
",
                    ]),
                )]),
                &[],
                &["`f` exposes `E`"],
            ),
            case(
                "generic callable argument in member and union",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"interface Callable<T> { (x: E): T }
export interface O { f: Callable<string> }
export declare const f: Callable<string> | undefined
",
                    ]),
                )]),
                &["`O` exposes `E`", "`f` exposes `E`"],
                &[],
            ),
            case(
                "eliminated conditional branch is not reported",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"export type Reduced = string extends string ? number : E
",
                    ]),
                )]),
                &[],
                &["`Reduced` exposes `E`"],
            ),
            case(
                "erased accessor alias",
                files(&[(
                    "test.ts",
                    r#"/** @stability experimental */
type ExperimentalText = string
export class C {
  get value(): ExperimentalText { return "x" }
}
"#,
                )]),
                &["`C` exposes `ExperimentalText`"],
                &[],
            ),
            case(
                "namespace member own stability",
                files(&[
                    (
                        "test.ts",
                        r#"export * as api from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        r"/** @stability experimental */
export interface E { value: string }
",
                    ),
                ]),
                &["`api` exposes `E`"],
                &[],
            ),
            case(
                "star reexport tag governs ceiling",
                files(&[
                    (
                        "test.ts",
                        r#"/** @stability experimental */
export * from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        r"/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }
",
                    ),
                ]),
                &[],
                &["exposes"],
            ),
            case(
                "star unstable reexport does not report experimental",
                files(&[
                    (
                        "test.ts",
                        r#"/** @stability unstable */
export * from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        r"/** @stability experimental */
export interface E { value: string }
export interface Leaky { value: E }
",
                    ),
                ]),
                &["`Leaky` exposes `E`"],
                &[],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:704 TestApiStabilityLeakAnonymousInstantiationColdBudget b486467ad04659fd
// TestApiStabilityLeakAnonymousInstantiationColdBudget pins the cold-budget P1
// blocker on a genuinely unprimed checker: the rule runs before any diagnostic
// snapshot, and the resulting diagnostics are compared with a separate control
// program that never ran the rule, so a TS2589 produced by the rule itself
// cannot be masked by pre-warming.
#[test]
fn test_api_stability_leak_anonymous_instantiation_cold_budget() {
    run(|t| {
        t.parallel();

        let files = files(&[(
            "test.ts",
            r"type Deep<T> = T extends string ? Deep<T> : T
declare function make<T>(): { value: Deep<T> }
export const object = make<string>()
",
        )]);

        for cold in [false, true] {
            t.run(&format!("cold={cold}"), |t| {
                t.parallel();
                let mut compiled = compile_api_stability_files_primed(t, &files, !cold);
                let (messages, delta, before_compiler, compiler) =
                    compiled.with_rule_ctx(|rule_ctx| {
                        let mut before_compiler = Vec::new();
                        if !cold {
                            before_compiler = api_stability_compiler_diagnostics(rule_ctx);
                        }
                        let before_instantiation = rule_ctx.checker().total_instantiation_count;
                        let messages = run_api_stability_messages(t, rule_ctx);
                        let delta =
                            rule_ctx.checker().total_instantiation_count - before_instantiation;
                        let compiler = api_stability_compiler_diagnostics(rule_ctx);
                        (messages, delta, before_compiler, compiler)
                    });
                drop(compiled);

                if delta > API_STABILITY_MAX_INSTANTIATION_DELTA {
                    t.error(format!(
                        "rule instantiated {delta} types, want at most {API_STABILITY_MAX_INSTANTIATION_DELTA}"
                    ));
                }
                if cold {
                    let control = api_stability_control_compiler_diagnostics(t, &files);
                    if control != compiler {
                        t.error(format!(
                            "cold rule changed compiler diagnostics against control:\ncontrol={control:?}\nafter={compiler:?}"
                        ));
                    }
                } else if before_compiler != compiler {
                    t.error(format!(
                        "rule changed compiler diagnostics:\nbefore={before_compiler:?}\nafter={compiler:?}"
                    ));
                }
                if !messages.is_empty() {
                    t.error(format!("got {} diagnostics, want 0: {messages:?}", messages.len()));
                }
            });
        }
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:746 TestApiStabilityLeakInspectionIsCycleSafe dd2bcab333fba99e
// TestApiStabilityLeakInspectionIsCycleSafe guards against unbounded recursion
// when a public surface refers back to itself through non-generic references.
#[test]
fn test_api_stability_leak_inspection_is_cycle_safe() {
    run(|t| {
        t.parallel();

        let mut compiled = compile_api_stability_files(
            t,
            &files(&[(
                "test.ts",
                r"/** @stability experimental */
interface E { value: string }
export interface Node { next: Node; payload: E }
",
            )]),
        );
        compiled.with_rule_ctx(|rule_ctx| {
            let before = rule_ctx.checker().total_instantiation_count;
            let messages = run_api_stability_messages(t, rule_ctx);
            let after = rule_ctx.checker().total_instantiation_count;

            if after != before {
                t.error(format!("rule instantiated {} types, want 0", after - before));
            }
            if !contains_api_stability_message(t, &messages, "`Node` exposes `E`") {
                t.fatal(format!("missing self-referential leak diagnostic: {messages:?}"));
            }
        });
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:769 TestApiStabilityLeakDoesNotTruncateMembers da11c82ef4b5d5df
// TestApiStabilityLeakDoesNotTruncateMembers covers the previous first-128
// member cutoff: a leak positioned after many harmless members is still found.
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

// go test: internal/rules/api_stability_leak_regression_test.go:792 TestApiStabilityLeakLongAliasChain ad9a18fea365c331
// TestApiStabilityLeakLongAliasChain covers a named re-export chain longer than
// the old 32-hop resolution cap. Alias resolution is cycle-safe by identity, so
// a 40-hop chain still forwards the public surface.
#[test]
fn test_api_stability_leak_long_alias_chain() {
    run(|t| {
        t.parallel();

        let mut files = files(&[
            ("test.ts", r#"export { Leaky } from "./barrel0""#),
            (
                "dep.ts",
                r"/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }
",
            ),
        ]);
        for index in 0..40 {
            let mut next = format!("barrel{}", index + 1);
            if index == 39 {
                next = "dep".to_string();
            }
            // Go: `fmt.Sprintf("export { Leaky } from %q", "./"+next)`; the module names need no escaping.
            files.push((
                format!("barrel{index}.ts"),
                format!("export {{ Leaky }} from \"./{next}\""),
            ));
        }

        assert_api_stability_sources_parse(t, &files);
        let mut compiled = compile_api_stability_files(t, &files);
        compiled.with_rule_ctx(|rule_ctx| {
            let before = rule_ctx.checker().total_instantiation_count;
            let messages = run_api_stability_messages(t, rule_ctx);
            let delta = rule_ctx.checker().total_instantiation_count - before;
            if delta != 0 {
                t.error(format!("rule instantiated {delta} types, want 0"));
            }
            if !contains_api_stability_message(t, &messages, "`Leaky` exposes `E`") {
                t.fatal(format!("40-hop alias chain lost its public surface: {messages:?}"));
            }
        });
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:829 TestApiStabilityLeakBudgetDiagnostics c932b07983a38c9e
// TestApiStabilityLeakBudgetDiagnostics reproduces the budget blocker
// directly: running the rule on the inferred accessor must neither add TS2589
// (source or global) nor scale compiler instantiations, on warm and genuinely
// cold checkers. The cold diagnostics are compared with a separate control.
#[test]
fn test_api_stability_leak_budget_diagnostics() {
    run(|t| {
        t.parallel();

        let files = files(&[(
            "test.ts",
            r"type Deep<T> = T extends string ? Deep<T> : T
interface Callable<T> { (x: Deep<T>): void }
declare const c: Callable<string>
export class Owner { get method() { return c } }
",
        )]);
        for cold in [false, true] {
            t.run(&format!("cold={cold}"), |t| {
                t.parallel();
                let mut compiled = compile_api_stability_files_primed(t, &files, !cold);
                let (messages, delta, before_compiler, after_compiler) =
                    compiled.with_rule_ctx(|rule_ctx| {
                        let mut before_compiler = Vec::new();
                        if !cold {
                            before_compiler = api_stability_compiler_diagnostics(rule_ctx);
                        }
                        let before_instantiation = rule_ctx.checker().total_instantiation_count;
                        let messages = run_api_stability_messages(t, rule_ctx);
                        let delta =
                            rule_ctx.checker().total_instantiation_count - before_instantiation;
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
                    let control = api_stability_control_compiler_diagnostics(t, &files);
                    if control != after_compiler {
                        t.error(format!(
                            "cold rule changed compiler diagnostics against control:\ncontrol={control:?}\nafter={after_compiler:?}"
                        ));
                    }
                } else if before_compiler != after_compiler {
                    t.error(format!(
                        "rule added compiler diagnostics:\nbefore={before_compiler:?}\nafter={after_compiler:?}"
                    ));
                }
                if !messages.is_empty() {
                    t.error(format!("got {} diagnostics, want 0: {messages:?}", messages.len()));
                }
            });
        }
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:876 TestApiStabilityLeakInheritedGenericAndCacheRegressions 47d1e46d682f74e6
// TestApiStabilityLeakInheritedGenericAndCacheRegressions pins inherited generic
// surface budget safety, substitution-context cache
// separation, sound cycle completion, nested named shallow surfaces, declared
// member/signature stability, replaced defaults, generic conditional branches,
// forwarding ceiling precedence and erased aliases inside unions and reference
// arguments. Order-sensitive cases run in both declaration orders and every
// case runs on warm and genuinely cold checkers.
#[test]
fn test_api_stability_leak_inherited_generic_and_cache_regressions() {
    run(|t| {
        t.parallel();

        let experimental_e = r"/** @stability experimental */
interface E { value: string }
"
        .to_string();
        let deep_alias = r"type Deep<T> = T extends string ? Deep<T> : T
"
        .to_string();
        let tests = vec![
            // P1: an inherited member of an uninstantiated interface can carry a
            // concrete recursive alias. It must be read from the base declaration
            // with a substitution, never by resolving the instantiated surface.
            case(
                "inherited generic member budget",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &deep_alias,
                        r"interface Base<T> { value: Deep<T> }
export interface Child extends Base<string> {}
",
                    ]),
                )]),
                &[],
                &["exposes"],
            ),
            case(
                "inherited generic signature budget",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &deep_alias,
                        r"interface Base<T> { (x: T): void }
interface Child<T> extends Base<Deep<T>> {}
declare function make<T>(): Child<T>
export const f = make<string>()
",
                    ]),
                )]),
                &[],
                &["exposes"],
            ),
            // P2: the same raw signature analyzed under different enclosing
            // substitutions must not share a cached result.
            case(
                "instantiated signature contexts clean first",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"declare function make<T>(): (x: T) => T
export const clean = make<string>()
export const leaky = make<E>()
",
                    ]),
                )]),
                &["`leaky` exposes `E`"],
                &["`clean` exposes `E`"],
            ),
            case(
                "instantiated signature contexts leaky first",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"declare function make<T>(): (x: T) => T
export const leaky = make<E>()
export const clean = make<string>()
",
                    ]),
                )]),
                &["`leaky` exposes `E`"],
                &["`clean` exposes `E`"],
            ),
            case(
                "nested instantiated contexts clean first",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"declare function make<T>(): { nested: { fn: (x: T) => T } }
export const clean = make<string>()
export const leaky = make<E>()
",
                    ]),
                )]),
                &["`leaky` exposes `E`"],
                &["`clean` exposes `E`"],
            ),
            case(
                "nested instantiated contexts leaky first",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"declare function make<T>(): { nested: { fn: (x: T) => T } }
export const leaky = make<E>()
export const clean = make<string>()
",
                    ]),
                )]),
                &["`leaky` exposes `E`"],
                &["`clean` exposes `E`"],
            ),
            // P2: an anonymous compound cycle must complete its dependency union
            // for every root rather than caching a cut result as complete.
            case(
                "mutual cycle A first",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"type B = { c: C; payload: E }
type C = { b: B }
export type A = { b: B }
export type Z = { c: C }
",
                    ]),
                )]),
                &["`A` exposes `E`", "`Z` exposes `E`"],
                &[],
            ),
            case(
                "mutual cycle Z first",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"type B = { c: C; payload: E }
type C = { b: B }
export type Z = { c: C }
export type A = { b: B }
",
                    ]),
                )]),
                &["`Z` exposes `E`", "`A` exposes `E`"],
                &[],
            ),
            // P2: a nested named interface is a shallow dependency; its arbitrary
            // members are not chased, while its public call signatures are.
            case(
                "nested named surface stays shallow",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"interface Hidden { value: E }
export declare const exposed: { hidden: Hidden }
",
                    ]),
                )]),
                &[],
                &["exposes"],
            ),
            case(
                "nested named callable signatures are inspected",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"interface Callable { (x: E): void }
export interface Holder { callable: Callable }
",
                    ]),
                )]),
                &["`Holder` exposes `E`"],
                &[],
            ),
            // P2: the computed minimum includes the declared stability of public
            // properties and call signatures, with provenance for naming.
            case(
                "declared member and signature minimum",
                files(&[(
                    "test.ts",
                    r"export interface O {
  /** @stability experimental */
  value: string
}
export interface S {
  /** @stability unstable */
  (x: string): string
}
",
                )]),
                &[
                    "`O` exposes `value`, an experimental API.",
                    "`S` exposes `call signature`, an unstable API.",
                ],
                &[],
            ),
            // P2: a default replaced by an explicit argument is not part of the
            // represented instantiated surface.
            case(
                "replaced generic default stays clean",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"declare function identity<T = E>(x: T): T
export const f = identity<string>
",
                    ]),
                )]),
                &[],
                &["exposes"],
            ),
            // Concrete-identity keys: the same raw member type is materialized with
            // different concrete arguments and must not contaminate either way.
            case(
                "shared raw target clean first",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"type Deep<T> = T extends string ? Deep<T> : T
interface DeepHolder<T> { value: Deep<T> }
export declare const clean: DeepHolder<string>
export declare const leaky: DeepHolder<E>
",
                    ]),
                )]),
                &["`leaky` exposes `E`"],
                &["`clean` exposes `E`"],
            ),
            case(
                "shared raw target leaky first",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"type Deep<T> = T extends string ? Deep<T> : T
interface DeepHolder<T> { value: Deep<T> }
export declare const leaky: DeepHolder<E>
export declare const clean: DeepHolder<string>
",
                    ]),
                )]),
                &["`leaky` exposes `E`"],
                &["`clean` exposes `E`"],
            ),
            case(
                "shared heritage materialization clean first",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"interface Base<T> { value: T }
export interface Clean extends Base<string> {}
export interface Leaky extends Base<E> {}
",
                    ]),
                )]),
                &["`Leaky` exposes `E`"],
                &["`Clean` exposes `E`"],
            ),
            case(
                "shared heritage materialization leaky first",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"interface Base<T> { value: T }
export interface Leaky extends Base<E> {}
export interface Clean extends Base<string> {}
",
                    ]),
                )]),
                &["`Leaky` exposes `E`"],
                &["`Clean` exposes `E`"],
            ),
            // P2: declared generic conditional branches are represented components.
            case(
                "generic conditional branch is inspected",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"export type Conditional<T> = T extends string ? E : number
",
                    ]),
                )]),
                &["`Conditional` exposes `E`"],
                &[],
            ),
            // P2: an explicit forwarding tag takes precedence over the forwarded
            // symbol's own ceiling, including when it is stricter.
            case(
                "named forwarding tag tightens ceiling",
                files(&[
                    (
                        "test.ts",
                        r#"/** @stability unstable */
export { Leaky } from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        r"/** @stability experimental */
interface E { value: string }
/** @stability experimental */
export interface Leaky { value: E }
",
                    ),
                ]),
                &["`Leaky` exposes `E`"],
                &[],
            ),
            case(
                "star forwarding tag tightens ceiling",
                files(&[
                    (
                        "test.ts",
                        r#"/** @stability unstable */
export * from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        r"/** @stability experimental */
interface E { value: string }
/** @stability experimental */
export interface Leaky { value: E }
",
                    ),
                ]),
                &["`Leaky` exposes `E`"],
                &[],
            ),
            case(
                "forwarding tag below target ceiling stays quiet",
                files(&[
                    (
                        "test.ts",
                        r#"/** @stability experimental */
export { Leaky } from "./dep"
"#,
                    ),
                    (
                        "dep.ts",
                        r"/** @stability experimental */
interface E { value: string }
/** @stability experimental */
export interface Leaky { value: E }
",
                    ),
                ]),
                &[],
                &["exposes"],
            ),
            // P2: erased aliases inside ordinary unions and reference arguments are
            // represented components and keep their provenance.
            case(
                "erased alias in union and reference argument",
                files(&[(
                    "test.ts",
                    r"/** @stability experimental */
type Text = string
interface Box<T> { value: T }
export declare function f(x: Text | undefined): void
export declare const g: Box<Text>
",
                )]),
                &["`f` exposes `Text`", "`g` exposes `Text`"],
                &[],
            ),
            case(
                "eliminated conditional branch stays clean",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &experimental_e,
                        r"export type Reduced = string extends string ? number : E
",
                    ]),
                )]),
                &[],
                &["exposes"],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:1130 TestApiStabilityLeakInstantiationDoesNotScaleWithDepth 9b6a3fd0d40cf4b5
// TestApiStabilityLeakInstantiationDoesNotScaleWithDepth is the core
// anti-instantiation guard: traversing a deeply nested reference such as
// `Box<Box<...<E>>>` must resolve each represented type argument exactly once,
// so the instantiation delta stays flat as depth grows instead of exploding.
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

// go test: internal/rules/api_stability_leak_regression_test.go:1164 TestApiStabilityLeakDeferredBranchRegressions 0795958598ea30fa
// TestApiStabilityLeakDeferredBranchRegressions pins non-instantiating
// declaration access for conditional branches and value
// annotations, substitution-aware branch selection, deferred-branch erased
// aliases and composite deferred checks. Every case runs on warm and genuinely
// cold checkers with the instantiation and diagnostic guards of
// runApiStabilityRegressionCase, so a recursive alias argument can neither be
// evaluated nor change the compiler's diagnostics.
#[test]
fn test_api_stability_leak_deferred_branch_regressions() {
    run(|t| {
        t.parallel();

        let deep_alias = "type Deep<T> = T extends string ? Deep<T> : T\n".to_string();
        let tests = vec![
            // A composite check that contains a type parameter keeps its
            // declared branches instead of being published blocked.
            case(
                "generic conditional array check",
                single_api_stability_file(r"export type C<T> = T[] extends string[] ? E : number"),
                &["`C` exposes `E`"],
                &[],
            ),
            // The represented substitution decides which branch survives; an
            // eliminated experimental branch is not reported.
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
            // The same selection holds for a named alias whose declared surface
            // is walked from its annotation on a cold checker.
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
            // A deferred branch that survives keeps the erased alias written in
            // its annotation.
            case(
                "deferred conditional erased branch alias",
                files(&[(
                    "test.ts",
                    r"/** @stability experimental */
type Text = string
export type C<T> = T extends number ? Text : boolean",
                )]),
                &["`C` exposes `Text`"],
                &[],
            ),
            // A declared reference whose argument is a recursive alias is never
            // resolved by the rule.
            case(
                "declared reference recursive argument budget",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &deep_alias,
                        r"interface Callable<T> { (x: T): void }
export declare const f: Callable<Deep<string>>",
                    ]),
                )]),
                &[],
                &["exposes"],
            ),
            // A deferred branch that instantiates a recursive alias is read
            // from its annotation, never evaluated.
            case(
                "branch recursive concrete alias budget",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &deep_alias,
                        r"export type C<T> = T extends number ? Deep<string> : boolean",
                    ]),
                )]),
                &[],
                &["exposes"],
            ),
            // The cold callable probe: an initializer call whose return type
            // carries a recursive alias argument is read through the resolved
            // signature's uninstantiated return type and concrete substitution.
            case(
                "recursive named callable argument budget",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &deep_alias,
                        r"interface Callable<T> { (x: T): void }
declare function make<T>(): Callable<Deep<T>>
export const f = make<string>()",
                    ]),
                )]),
                &[],
                &["exposes"],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:1249 TestApiStabilityLeakCyclicCompletionIsReusable 350724ea05e6b699
// TestApiStabilityLeakCyclicCompletionIsReusable compiles the mutual anonymous
// cycle in both export orders, repeatedly and on warm checkers, and requires
// every root to report the experimental dependency. A cut result must never be
// reported as stable, and a settled result must be reusable within its session.
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

// go test: internal/rules/api_stability_leak_regression_test.go:1279 TestApiStabilityNonInstantiatingAccessors 7aa9b6b02d8c1590
// TestApiStabilityNonInstantiatingAccessors pins that the checker accessors the
// analysis hides behind only peek cached fields: a conditional branch, an index
// signature value and a symbol or alias type are never resolved, so a recursive
// alias argument such as `Deep<string>` cannot be instantiated by them.
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

// go test: internal/rules/api_stability_leak_regression_test.go:1375 TestApiStabilityLeakConditionalApplicationRegressions 92a7ee28688c6e0f
// TestApiStabilityLeakConditionalApplicationRegressions pins annotated
// recursive declarations are read without evaluating a generic alias, deferred
// conditional applications distinguish concrete substitutions, declared member
// and signature tags contribute on cold checkers, `typeof` callable surfaces
// keep their signature dependencies, and eliminated `string[]`/`never`
// conditional branches stay clean. Every case runs on warm and genuinely cold
// checkers with the instantiation and compiler-diagnostic guards of
// runApiStabilityRegressionCase.
#[test]
fn test_api_stability_leak_conditional_application_regressions() {
    run(|t| {
        t.parallel();

        let deep_alias = "type Deep<T> = T extends string ? Deep<T> : T\n".to_string();
        let step_alias = "type Step<T extends unknown[]> = T extends [unknown, ...infer Tail] ? Step<Tail> : number\n".to_string();
        let tests = vec![
            // P1: an annotated recursive declaration is never resolved, on any
            // declaration surface (parameter, return, member, inherited member).
            case(
                "annotated recursive parameter",
                files(&[(
                    "test.ts",
                    &cat(&[&deep_alias, r"export declare function f(x: Deep<string>): void"]),
                )]),
                &[],
                &["exposes"],
            ),
            case(
                "annotated recursive return",
                files(&[(
                    "test.ts",
                    &cat(&[&deep_alias, r"export declare function f(): Deep<string>"]),
                )]),
                &[],
                &["exposes"],
            ),
            case(
                "annotated recursive declared property",
                files(&[(
                    "test.ts",
                    &cat(&[&deep_alias, r"export interface I { value: Deep<string> }"]),
                )]),
                &[],
                &["exposes"],
            ),
            case(
                "annotated recursive inherited method",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &deep_alias,
                        r"interface Base { f(x: Deep<string>): void }",
                        "\n",
                        r"export interface Child extends Base {}",
                    ]),
                )]),
                &[],
                &["exposes"],
            ),
            // A finite recursive tuple application stays deferred as well; it
            // must not be instantiated just because it terminates.
            case(
                "finite recursive tuple application",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &step_alias,
                        r"export declare function f(x: Step<[",
                        &"0,".repeat(60),
                        r"]>): void",
                    ]),
                )]),
                &[],
                &["exposes"],
            ),
            // P2: distinct concrete applications of a recursive alias are
            // distinguished, so the eliminated branch of the finite alias is still
            // inspected on a cold checker.
            case(
                "finite recursive alias application",
                single_api_stability_file(
                    r"type A<T> = T extends string ? A<number> : E
export declare const x: A<string>",
                ),
                &["`x` exposes `E`"],
                &[],
            ),
            // A generic alias whose right-hand side is a named interface keeps
            // the concrete argument when the referenced declaration is read.
            case(
                "alias of interface preserves argument",
                single_api_stability_file(
                    r"interface Box<T> { value: T }
type Alias<T> = Box<T>
export declare const x: Alias<E>",
                ),
                &["`x` exposes `E`"],
                &[],
            ),
            case(
                "alias of type literal preserves argument",
                single_api_stability_file(
                    r"type Wrapper<T> = { value: T }
export declare const y: Wrapper<E>",
                ),
                &["`y` exposes `E`"],
                &[],
            ),
            // An application the checker evaluates to `never` exposes neither
            // its operands nor its branches.
            case(
                "eliminated conditional alias application",
                single_api_stability_file(
                    r"type Hidden<T> = T extends string ? E : never
export declare const clean: Hidden<E>",
                ),
                &[],
                &["exposes"],
            ),
            // P2: declared member and signature tags contribute even when the
            // checker has not materialized the annotation they belong to.
            case(
                "cold tagged anonymous property",
                files(&[(
                    "test.ts",
                    r"export declare const o: {
/** @stability experimental */
value: string
}",
                )]),
                &["exposes"],
                &[],
            ),
            case(
                "cold tagged anonymous method",
                files(&[(
                    "test.ts",
                    r"export declare const o: {
/** @stability experimental */
f(x: string): void
}",
                )]),
                &["exposes"],
                &[],
            ),
            case(
                "cold tagged anonymous call signature",
                files(&[(
                    "test.ts",
                    r"export declare const o: {
/** @stability experimental */
(x: string): void
}",
                )]),
                &["exposes"],
                &[],
            ),
            case(
                "cold tagged conditional branch member",
                files(&[(
                    "test.ts",
                    r"export type C<T> = T extends number ? {
/** @stability experimental */
value: string
} : boolean",
                )]),
                &["exposes"],
                &[],
            ),
            // P2: a `typeof` callable keeps its public signature dependencies
            // without resolving the referenced function's parameter recursively.
            case(
                "cold typeof callable",
                single_api_stability_file(&cat(&[
                    r"declare function fn(x: E): void",
                    "\n",
                    r"export declare const o: typeof fn",
                ])),
                &["`o` exposes `E`"],
                &[],
            ),
            // The same callable surface keeps its budget when the referenced
            // signature parameter is a recursive alias application.
            case(
                "cold typeof recursive callable",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &deep_alias,
                        r"declare function fn(x: Deep<string>): void",
                        "\n",
                        r"export declare const o: typeof fn",
                    ]),
                )]),
                &[],
                &["exposes"],
            ),
            // A tagged member of an anonymous surface is reported even when its
            // own type is a recursive alias application that must stay deferred.
            case(
                "cold tagged anonymous recursive member",
                files(&[(
                    "test.ts",
                    &cat(&[
                        &deep_alias,
                        r"export declare const o: {
/** @stability experimental */
value: Deep<string>
}",
                    ]),
                )]),
                &["exposes"],
                &[],
            ),
            // P2: a concrete compound substitution is preserved so an eliminated
            // conditional branch is not reported; `never` distributes to no branch.
            case(
                "compound argument conditional eliminated",
                single_api_stability_file(&cat(&[
                    r"interface Holder<T> { value: T extends string[] ? number : E }",
                    "\n",
                    r"export declare const clean: Holder<string[]>",
                ])),
                &[],
                &["exposes"],
            ),
            case(
                "never distributive conditional eliminated",
                single_api_stability_file(&cat(&[
                    r"interface Holder<T> { value: T extends string ? E : number }",
                    "\n",
                    r"export declare const clean: Holder<never>",
                ])),
                &[],
                &["exposes"],
            ),
        ];

        run_api_stability_regression_cases(t, &tests);
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:1539 TestApiStabilityLeakColdLazyReadMatchesWarm 05218912e3bdaf03
// TestApiStabilityLeakColdLazyReadMatchesWarm proves a cold public surface is
// materialized through the checker's ordinary lazy reads and agrees with the
// materialized recomputation: `Callable<Deep<E>>` resolves E on the cold
// checker (the application terminates because E is not a string), and the
// warmed recomputation reports the same dependency from a fresh analysis.
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

// go test: internal/rules/api_stability_leak_regression_test.go:1574 containsDependencyName 9003d8007b11a675
// containsDependencyName reports whether a computed surface lists the named
// dependency, used by the cold/warm recompute regression.
fn contains_dependency_name(used: &ApiStabilityUsed, name: &str) -> bool {
    for (_, dependency) in used.dependencies().all() {
        if api_stability_dependency_name(dependency) == name {
            return true;
        }
    }
    false
}

// go test: internal/rules/api_stability_leak_regression_test.go:1587 apiStabilityColdUnavailableProbes 3d5165453f638fd9
// apiStabilityColdUnavailableProbes groups the cold-unavailable probes whose
// public types are unavailable on a genuinely unprimed checker. None of them
// may trigger checker instantiation, produce a compiler diagnostic, report a
// fabricated dependency, or publish a complete cache entry.
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

// go test: internal/rules/api_stability_leak_regression_test.go:1623 TestApiStabilityLeakColdUnavailableIsSafe 326928ebcd4b757a
// TestApiStabilityLeakColdUnavailableIsSafe runs every recursive probe whose
// public type is not materialized on an unprimed checker. The analysis must do
// zero recursive work: no checker instantiation, no global diagnostic, no
// fabricated dependency, and no analysis state that survives the run.
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

// go test: internal/rules/api_stability_leak_regression_test.go:1657 TestApiStabilityLeakWarmReanalysisIsExact 3eff93d055ef00f9
// TestApiStabilityLeakWarmReanalysisIsExact re-runs the same probes on a
// checked checker, where the normal rule lifecycle makes the public types
// available. Each repro must then report exactly its expected result.
#[test]
fn test_api_stability_leak_warm_reanalysis_is_exact() {
    run(|t| {
        t.parallel();

        let prefix = "/** @stability experimental */\ninterface E { value: string }\n";
        // Rust: Go's anonymous `struct{name, source, want, notWant}` cases.
        let cases: Vec<(&str, &str, &[&str], &[&str])> = vec![
            (
                "typeof inferred arrow",
                r"declare const e: E
const fn = () => e
export declare const x: typeof fn",
                &["`x` exposes `E`"],
                &[],
            ),
            (
                "typeof inferred nested closure",
                r"declare const e: E
const fn = () => () => e
export declare const x: typeof fn",
                &["`x` exposes `E`"],
                &[],
            ),
            (
                "indexed erased dependency",
                r#"interface I { value: E }
type Read<T> = T["value" & keyof T]
export declare const x: Read<I>"#,
                &["`x` exposes `E`"],
                &[],
            ),
            (
                "indexed recursive parameter stays clean",
                r#"type Deep<T> = T extends string ? Deep<T> : T
interface I { x: Deep<string> }
export declare function f(x: I["x"]): void"#,
                &[],
                &["exposes"],
            ),
            (
                "anonymous recursive index stays clean",
                r"type Deep<T> = T extends string ? Deep<T> : T
export declare function f(): { [key: string]: Deep<string> }",
                &[],
                &["exposes"],
            ),
            (
                "recursive generic default parameter stays clean",
                r"type Deep<T> = T extends string ? Deep<T> : T
interface I<T = Deep<string>> { x: T }
export declare function f(x: I): void",
                &[],
                &["exposes"],
            ),
            (
                "recursive generic default return stays clean",
                r"type Deep<T> = T extends string ? Deep<T> : T
interface I<T = Deep<string>> { x: T }
export declare function f(): I",
                &[],
                &["exposes"],
            ),
            (
                "default conditional interface stays clean",
                r"interface Holder<T = string> { value: T extends string ? number : E }
export declare const x: Holder",
                &[],
                &["`x` exposes `E`"],
            ),
            (
                "default conditional alias stays clean",
                r"type Holder<T = string> = T extends string ? number : E
export declare const x: Holder",
                &[],
                &["`x` exposes `E`"],
            ),
            (
                "eliminated conditional alias application stays clean",
                r"type Hidden<T> = T extends string ? E : never
export declare const clean: Hidden<E>",
                &[],
                &["exposes"],
            ),
            (
                "finite recursive alias application reports",
                r"type A<T> = T extends string ? A<number> : E
export declare const x: A<string>",
                &["`x` exposes `E`"],
                &[],
            ),
        ];

        for (name, source, want, not_want) in cases {
            t.run(name, |t| {
                t.parallel();
                let mut compiled = compile_api_stability_files_primed(
                    t,
                    &files(&[("test.ts", &cat(&[prefix, source]))]),
                    true,
                );
                compiled.with_rule_ctx(|rule_ctx| {
                    let before = api_stability_compiler_diagnostics(rule_ctx);
                    let messages = run_api_stability_messages(t, rule_ctx);
                    let after = api_stability_compiler_diagnostics(rule_ctx);
                    if before != after {
                        t.fatal(format!(
                            "warm rule changed compiler diagnostics:\nbefore={before:?}\nafter={after:?}"
                        ));
                    }
                    for want in want {
                        if !contains_api_stability_message(t, &messages, want) {
                            t.error(format!("missing diagnostic {want:?} in {messages:?}"));
                        }
                    }
                    for not_want in not_want {
                        if contains_api_stability_message(t, &messages, not_want) {
                            t.error(format!("unexpected diagnostic {not_want:?} in {messages:?}"));
                        }
                    }
                });
            });
        }
    });
}

// go test: internal/rules/api_stability_leak_regression_test.go:1772 TestApiStabilityLeakRepresentedConditionalOutcomes 4d3bb60e8d759d6c
// TestApiStabilityLeakRepresentedConditionalOutcomes proves the analysis uses a
// conditional outcome the checker has materialized instead of guessing a
// branch. The instantiated member type is requested first, the way a consumer
// of the exported value would, and the rule must then reproduce the compiler's
// selection for contravariant, invariant, array-wrapped and structural checks.
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

// go test: internal/rules/api_stability_leak_regression_test.go:1850 TestApiStabilityLeakLazyConditionalOutcomes 4277534e89369b34
// TestApiStabilityLeakLazyConditionalOutcomes proves the corrected lazy-read
// boundary for instantiated conditional members: the concrete member is
// materialized through the checker's ordinary accessor, so the selected branch
// is the compiler's outcome rather than a guess. A contravariant check selects
// the experimental branch and reports; a structural check selects the clean
// branch and stays quiet. Both outcomes are complete surfaces, on warm and
// genuinely cold checkers, and a cold run never changes the compiler's own
// diagnostics.
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

// go test: internal/rules/api_stability_leak_regression_test.go:1916 TestApiStabilityLeakDeclaredIndexSignatureTags f991e5e31a16039f
// TestApiStabilityLeakDeclaredIndexSignatureTags pins the index signature tag
// fix: the signature's own `@stability` tag contributes on both the warm
// represented path and the cold metadata path, for a declared interface and an
// inline type literal.
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

// go test: internal/rules/api_stability_leak_regression_test.go:1966 TestApiStabilityLeakFiniteRecursionDoesNotInstantiate 74f5aaab85538827
// TestApiStabilityLeakFiniteRecursionDoesNotInstantiate is the
// finite-recursion anti-instantiation guard for valid finite recursion: a 60-element `Step`
// application reached through an indexed parameter or through inferred getter,
// parameter, return and nested object surfaces must not make the rule
// instantiate anything on an unprimed checker, and must not produce a compiler
// diagnostic.
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
