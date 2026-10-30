// Port of internal/rulerunner/api_stability_leak_test.go.

use std::sync::Arc;

use ets_etscore::{EffectPluginOptions, Severity};
use ets_go::testing::T;
use ets_go::{sprintf, sprintln};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, SourceFile, Symbol};
use ets_shim::checker::Checker;
use ets_shim::compiler::{CheckerHandle, Program, ProgramOptions};
use ets_shim::core::context::Context;
use ets_shim::core::tspath::ComparePathsOptions;
use ets_shim::core::{CompilerOptions, ModuleKind, ModuleResolutionKind, ScriptTarget, Tristate};
use ets_shim::vfs::{FS, bundled, vfstest};
use ets_typeparser::{ApiStabilityLevel, ApiStabilityUsed};
use rustc_hash::FxHashMap;

// go test: internal/rulerunner/api_stability_leak_test.go:27 apiStabilityLifecycleOptions 28267f3a3e75b8ff
pub(crate) fn api_stability_lifecycle_options(enabled: bool) -> EffectPluginOptions {
    let mut severity = FxHashMap::default();
    for r in ets_rules::ALL.iter() {
        severity.insert(r.name.to_string(), Severity::Off);
    }
    if enabled {
        severity.insert("apiStabilityLeak".to_string(), Severity::Warning);
    }
    EffectPluginOptions {
        diagnostics: true,
        diagnostic_severity: Some(severity),
        ..Default::default()
    }
}

// go test: internal/rulerunner/api_stability_leak_test.go:38 apiStabilityLifecycleDiagnostics 6a723b2e84bc99e5
// Rust: `files` is a slice of (name, source) pairs (Go ranges over a map, then sorts the names).
pub(crate) fn api_stability_lifecycle_diagnostics(
    t: &T,
    files: &[(&str, &str)],
    enabled: bool,
    whole_program: bool,
) -> Vec<String> {
    t.helper();
    ets_etscheckerhooks::init();

    let mut fsmap = Vec::new();
    let mut names = Vec::new();
    for (name, source) in files {
        let path = format!("/.src/{name}");
        fsmap.push((path.clone(), source.to_string()));
        names.push(path);
    }
    names.sort();

    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(fsmap, true)));
    let options = CompilerOptions {
        target: ScriptTarget::ES_NEXT,
        module: ModuleKind::NODE_NEXT,
        module_resolution: ModuleResolutionKind::NODE_NEXT,
        strict: Tristate::True,
        skip_lib_check: Tristate::True,
        skip_default_lib_check: Tristate::True,
        effect: Some(Arc::new(api_stability_lifecycle_options(enabled))).into(),
        ..Default::default()
    };
    let host =
        ets_shim::compiler::new_compiler_host("/.src", fs, &bundled::lib_path(), None, None, None);
    let config = ets_shim::tsoptions::new_parsed_command_line(
        P::new(options),
        names,
        Vec::new(),
        ComparePathsOptions {
            use_case_sensitive_file_names: true,
            current_directory: "/.src".to_string(),
        },
    );
    let mut program_options = ProgramOptions::new(P::new(config), host);
    program_options.single_threaded = Tristate::True;
    let program = crate::new_test_program(t, program_options);
    let ctx = Context::background();
    let Some(sf) = program.get_source_file("/.src/test.ts") else {
        t.fatal("missing test.ts");
    };

    let mut messages = Vec::new();
    let mut add = |diagnostic: P<Diagnostic>| {
        if let Some(diagnostic_file) = diagnostic.file() {
            messages.push(format!("{}:{}", diagnostic_file.file_name(), diagnostic.localize()));
            return;
        }
        messages.push(diagnostic.localize());
    };
    if whole_program {
        for diagnostic in program.get_semantic_diagnostics(&ctx, None) {
            add(diagnostic);
        }
        for diagnostic in program.get_global_diagnostics(&ctx) {
            add(diagnostic);
        }
    } else {
        let mut checker = program.get_type_checker(&ctx);
        for diagnostic in checker.get_diagnostics_exported(&ctx, sf) {
            add(diagnostic);
        }
        for diagnostic in checker.get_global_diagnostics() {
            add(diagnostic);
        }
        drop(checker);
    }
    messages
}

// go test: internal/rulerunner/api_stability_leak_test.go:103 apiStabilityLifecycleContains edacfd637d66f4aa
pub(crate) fn api_stability_lifecycle_contains(messages: &[String], want: &str) -> bool {
    for message in messages {
        if message.contains(want) {
            return true;
        }
    }
    false
}

/// Rust-only: borrows a table case's owned `(name, source)` pairs as the `&[(&str, &str)]` the helpers take
/// (Go's `map[string]string`).
fn borrow_files<'a>(files: &'a [(&'a str, String)]) -> Vec<(&'a str, &'a str)> {
    files.iter().map(|(name, source)| (*name, source.as_str())).collect()
}

/// Rust-only: Go's `strings.Contains(message, "effect(apiStabilityLeak)")`, repeated by every case.
fn is_rule_message(message: &str) -> bool {
    message.contains("effect(apiStabilityLeak)")
}

// go test: internal/rulerunner/api_stability_leak_test.go:112 TestApiStabilityLeakLifecycleRecursiveSafety 7d2f8ba03c0b0d72
#[test]
fn test_api_stability_leak_lifecycle_recursive_safety() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = format!(
            "{API_STABILITY_RECURSIVE_PREFIX}declare const key: Deep<string>\nexport interface X {{ [key]: number }}\n"
        );
        let files = [("test.ts", source.as_str())];
        let messages = api_stability_lifecycle_diagnostics(t, &files, true, true);
        let control = api_stability_lifecycle_diagnostics(t, &files, false, true);
        if messages.len() != control.len() {
            t.error(sprintf!(
                "rule changed diagnostics for an already-erroring Deep input:\nrule=%v\ncontrol=%v",
                messages,
                control
            ));
        }
        if control.is_empty() {
            t.fatal(sprintln!("expected the control to report the baseline TS2589"));
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:127 apiStabilityRecursivePrefix 165c09ee2c2db74b
const API_STABILITY_RECURSIVE_PREFIX: &str = "type Deep<T> = T extends string ? Deep<T> : T\n";

// go test: internal/rulerunner/api_stability_leak_test.go:129 runApiStabilitySafetyCase 6cb2ca3c8beb9b3a
fn run_api_stability_safety_case(
    t: &T,
    name: &str,
    files: &[(&str, &str)],
    want: &[&str],
    not_want: &[&str],
) {
    t.helper();
    for whole_program in [false, true] {
        let mode = if whole_program { "program" } else { "file" };
        t.run(&format!("{name}/{mode}"), |t| {
            t.parallel();
            let messages = api_stability_lifecycle_diagnostics(t, files, true, whole_program);
            let control = api_stability_lifecycle_diagnostics(t, files, false, whole_program);
            for message in &messages {
                if is_rule_message(message) {
                    continue;
                }
                if !api_stability_lifecycle_contains(&control, message) {
                    t.error(sprintf!(
                        "rule introduced compiler diagnostic %q\nrule=%v\ncontrol=%v",
                        message,
                        messages,
                        control
                    ));
                }
            }
            for message in &control {
                if !api_stability_lifecycle_contains(&messages, message) {
                    t.error(sprintf!(
                        "rule lost compiler diagnostic %q\nrule=%v\ncontrol=%v",
                        message,
                        messages,
                        control
                    ));
                }
            }
            for expected in want {
                if !api_stability_lifecycle_contains(&messages, expected) {
                    t.error(sprintf!("missing %q in %v", expected, messages));
                }
            }
            for forbidden in not_want {
                if api_stability_lifecycle_contains(&messages, forbidden) {
                    t.error(sprintf!("unexpected %q in %v", forbidden, messages));
                }
            }
        });
    }
}

// go test: internal/rulerunner/api_stability_leak_test.go:167 apiStabilityProgram 4188662c9b57bd65
// Rust: the checker handle is the cleanup (dropping it is Go's `done()`).
fn api_stability_program(
    t: &T,
    files: &[(&str, &str)],
    enabled: bool,
) -> (&'static Program, CheckerHandle) {
    t.helper();
    ets_etscheckerhooks::init();
    let mut fsmap = Vec::new();
    let mut names = Vec::new();
    for (name, source) in files {
        let path = format!("/.src/{name}");
        fsmap.push((path.clone(), source.to_string()));
        names.push(path);
    }
    names.sort();
    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(fsmap, true)));
    let options = CompilerOptions {
        effect: Some(Arc::new(api_stability_lifecycle_options(enabled))).into(),
        target: ScriptTarget::ES_NEXT,
        module: ModuleKind::NODE_NEXT,
        module_resolution: ModuleResolutionKind::NODE_NEXT,
        strict: Tristate::True,
        skip_lib_check: Tristate::True,
        skip_default_lib_check: Tristate::True,
        no_error_truncation: Tristate::True,
        ..Default::default()
    };
    let host =
        ets_shim::compiler::new_compiler_host("/.src", fs, &bundled::lib_path(), None, None, None);
    let config = ets_shim::tsoptions::new_parsed_command_line(
        P::new(options),
        names,
        Vec::new(),
        ComparePathsOptions {
            use_case_sensitive_file_names: true,
            current_directory: "/.src".to_string(),
        },
    );
    let mut program_options = ProgramOptions::new(P::new(config), host);
    program_options.single_threaded = Tristate::True;
    let program = crate::new_test_program(t, program_options);
    let c = program.get_type_checker(&Context::background());
    (program, c)
}

// go test: internal/rulerunner/api_stability_leak_test.go:198 apiStabilityExport fc5354e37580c2b3
fn api_stability_export(t: &T, c: &mut Checker, sf: P<SourceFile>, name: &str) -> P<Symbol> {
    t.helper();
    let module_symbol = ets_typeparser::get_symbol_of_declaration(c, sf.as_node());
    if let Some(module_symbol) = module_symbol {
        for symbol in c.get_exports_of_module_exported(module_symbol) {
            if symbol.name() == name {
                return symbol;
            }
        }
    }
    t.fatal(sprintf!("missing export %s", name));
}

/// Rust-only: Go's `program.GetSourceFile(name)` on a file the test created (Go would crash on the nil file later).
fn source_file(program: &'static Program, name: &str) -> P<SourceFile> {
    program.get_source_file(name).unwrap_or_else(|| panic!("missing source file {name}"))
}

// go test: internal/rulerunner/api_stability_leak_test.go:210 apiStabilityDiagnostics 9bee25eb6adb5ffb
fn api_stability_diagnostics(t: &T, c: &mut Checker, sf: P<SourceFile>) -> Vec<String> {
    t.helper();
    let mut out = Vec::new();
    for diagnostic in c.get_diagnostics_exported(&Context::background(), sf) {
        let text = diagnostic.localize();
        if text.is_empty() {
            continue;
        }
        if is_rule_message(&text) {
            continue;
        }
        out.push(format!("{}:{}", diagnostic.code(), text));
    }
    for diagnostic in c.get_global_diagnostics() {
        out.push(format!("global:{}:{}", diagnostic.code(), diagnostic.localize()));
    }
    out.sort();
    out
}

// go test: internal/rulerunner/api_stability_leak_test.go:232 apiStabilityProgramDiagnostics 080b308b15cc6703
fn api_stability_program_diagnostics(t: &T, program: &'static Program) -> Vec<String> {
    t.helper();
    let mut out = Vec::new();
    let mut add = |diagnostic: P<Diagnostic>| {
        let text = diagnostic.localize();
        if text.is_empty() {
            return;
        }
        if is_rule_message(&text) {
            return;
        }
        out.push(format!("{}:{}", diagnostic.code(), text));
    };
    let ctx = Context::background();
    for diagnostic in program.get_semantic_diagnostics(&ctx, None) {
        add(diagnostic);
    }
    for diagnostic in program.get_global_diagnostics(&ctx) {
        add(diagnostic);
    }
    out.sort();
    out
}

/// Rust-only: Go's `program, _, done = apiStabilityProgram(t, files, enabled)`, then
/// `apiStabilityProgramDiagnostics(t, program)` and `done()`. The checker handle is released before the program
/// computes its own diagnostics, which take the same checker (holding it would deadlock).
fn api_stability_whole_program_diagnostics(
    t: &T,
    files: &[(&str, &str)],
    enabled: bool,
) -> Vec<String> {
    let (program, c) = api_stability_program(t, files, enabled);
    drop(c);
    api_stability_program_diagnostics(t, program)
}

// go test: internal/rulerunner/api_stability_leak_test.go:254 TestApiStabilityLeakWholeSurfaceSafety 9dae35346859ba32
#[test]
fn test_api_stability_leak_whole_surface_safety() {
    ets_go::testing::run(|t| {
        t.parallel();
        let p = |s: &str| format!("{API_STABILITY_RECURSIVE_PREFIX}{s}");

        struct Case {
            name: &'static str,
            files: Vec<(&'static str, String)>,
            not_want: Vec<&'static str>,
        }
        let cases = [
            Case {
                name: "recursive member",
                files: vec![(
                    "test.ts",
                    p(
                        "interface Wrapper<T> { value: Deep<T> }\nexport declare const exposed: Wrapper<string>\n",
                    ),
                )],
                not_want: vec![],
            },
            Case {
                name: "recursive index value",
                files: vec![(
                    "test.ts",
                    p(
                        "interface I<T> { [s: string]: Deep<T> }\nexport declare const x: I<string>\n",
                    ),
                )],
                not_want: vec![],
            },
            Case {
                name: "inherited recursive index value",
                files: vec![(
                    "test.ts",
                    p(
                        "interface Base<T> { [s: string]: Deep<T> }\ninterface I<T> extends Base<T> {}\nexport declare const x: I<string>\n",
                    ),
                )],
                not_want: vec![],
            },
            Case {
                name: "anonymous recursive index return",
                files: vec![(
                    "test.ts",
                    p(
                        "declare function make<T>(): { [s: string]: Deep<T> }\nexport const x = make<string>()\n",
                    ),
                )],
                not_want: vec![],
            },
            Case {
                name: "compound indexed callable return",
                files: vec![(
                    "test.ts",
                    p(
                        "interface I<T> { v: Deep<T> }\ndeclare function make<T>(): () => T[\"v\" & keyof T]\nexport const x = make<I<string>>()\n",
                    ),
                )],
                not_want: vec![],
            },
            Case {
                name: "compound indexed parameter",
                files: vec![(
                    "test.ts",
                    p(
                        "interface I<T> { v: Deep<T> }\ndeclare function make<T>(): (x: T[\"v\" & keyof T]) => void\nexport const x = make<I<string>>()\n",
                    ),
                )],
                not_want: vec![],
            },
            Case {
                name: "compound indexed nested return",
                files: vec![(
                    "test.ts",
                    p(
                        "interface I<T> { v: Deep<T> }\ndeclare function make<T>(): () => { value: T[\"v\" & keyof T] }\nexport const x = make<I<string>>()\n",
                    ),
                )],
                not_want: vec![],
            },
            Case {
                name: "compound indexed alias return",
                files: vec![(
                    "test.ts",
                    p(
                        "interface I<T> { v: Deep<T> }\ntype Read<T> = T[\"v\" & keyof T]\ndeclare function make<T>(): () => Read<T>\nexport const x = make<I<string>>()\n",
                    ),
                )],
                not_want: vec![],
            },
            Case {
                name: "wide tuple recursive member",
                files: vec![(
                    "test.ts",
                    p(&format!(
                        "interface I<T> {{\n  v: [{}Deep<T>]\n}}\nexport declare const x: I<string>\n",
                        "number,".repeat(4100)
                    )),
                )],
                not_want: vec![],
            },
            Case {
                name: "skipped declaration alias default",
                files: vec![
                    ("test.ts", "export * as ns from \"./dep.js\"\n".to_string()),
                    ("dep.d.ts", p("type D<T = Deep<string>> = T\nexport interface X { v: D }\n")),
                ],
                not_want: vec![],
            },
            Case {
                name: "generic default references earlier parameter",
                files: vec![(
                    "test.ts",
                    p("interface I<A, B = Deep<A>> { v: B }\nexport declare const x: I<string>\n"),
                )],
                not_want: vec![],
            },
            Case {
                name: "internal export behind a star cycle",
                files: vec![
                    (
                        "test.ts",
                        p(
                            "/** @internal */\nexport interface Cyclic { value: Deep<string> }\nexport * from \"./dep\"\n",
                        ),
                    ),
                    ("dep.ts", "export * from \"./test\"\n".to_string()),
                ],
                not_want: vec!["`Cyclic` exposes"],
            },
            Case {
                name: "recursive inferred accessor",
                files: vec![
                    ("test.ts", "export { f } from \"./dep.js\"".to_string()),
                    (
                        "dep.ts",
                        p(
                            "declare function make<T>(): Deep<T>\nexport const f = { get v() { return make<string>() } }",
                        ),
                    ),
                ],
                not_want: vec!["exposes"],
            },
            Case {
                name: "recursive class accessor",
                files: vec![
                    ("test.ts", "export { C } from \"./dep.js\"".to_string()),
                    (
                        "dep.ts",
                        p(
                            "declare const deep: Deep<string>\nexport class C { get value() { return deep } }",
                        ),
                    ),
                ],
                not_want: vec!["exposes"],
            },
        ];
        for test in &cases {
            run_api_stability_safety_case(
                t,
                test.name,
                &borrow_files(&test.files),
                &[],
                &test.not_want,
            );
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:367 TestApiStabilityLeakBareBinderOrders f04aaf0edf9a606c
#[test]
fn test_api_stability_leak_bare_binder_orders() {
    ets_go::testing::run(|t| {
        t.parallel();

        let orders = [
            (
                "clean first",
                "/** @stability experimental */
interface E { v: string }
declare function make<T>(): () => T
export const clean = make<string>()
export const leaky = make<E>()
",
            ),
            (
                "leaky first",
                "/** @stability experimental */
interface E { v: string }
declare function make<T>(): () => T
export const leaky = make<E>()
export const clean = make<string>()
",
            ),
        ];
        for (name, source) in orders {
            t.run(name, |t| {
                t.parallel();
                let (program, mut c) = api_stability_program(t, &[("test.ts", source)], true);
                let sf = source_file(program, "/.src/test.ts");
                let _ = c.get_diagnostics_exported(&Context::background(), sf);
                {
                    let mut tp = ets_typeparser::new_type_parser(program, &mut c);
                    for i in 0..5_i64 {
                        let clean_symbol = api_stability_export(t, tp.checker(), sf, "clean");
                        let clean = tp.api_stability_used_by_symbol(clean_symbol);
                        let leaky_symbol = api_stability_export(t, tp.checker(), sf, "leaky");
                        let leaky = tp.api_stability_used_by_symbol(leaky_symbol);
                        if clean.minimum() != ApiStabilityLevel::Stable {
                            t.fatal(sprintf!(
                                "query %d: clean minimum=%d, want stable",
                                i,
                                clean.minimum() as i64
                            ));
                        }
                        if leaky.minimum() != ApiStabilityLevel::Experimental {
                            t.fatal(sprintf!(
                                "query %d: leaky minimum=%d, want experimental",
                                i,
                                leaky.minimum() as i64
                            ));
                        }
                    }
                }
                let diagnostics = api_stability_diagnostics(t, &mut c, sf);
                if !diagnostics.is_empty() {
                    t.fatal(sprintf!("rule changed compiler diagnostics: %v", diagnostics));
                }
            });
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:417 TestApiStabilityLeakSubsequentCheckerExactness 9a5effc0247d42ed
#[test]
fn test_api_stability_leak_subsequent_checker_exactness() {
    ets_go::testing::run(|t| {
        t.parallel();
        let p = |s: &str| format!("{API_STABILITY_RECURSIVE_PREFIX}{s}");

        let cases: Vec<(&str, Vec<(&str, String)>)> = vec![
            (
                "recursive index namespace barrel",
                vec![
                    ("test.ts", r#"export * as ns from "./dep.js""#.to_string()),
                    (
                        "dep.d.ts",
                        p(
                            "export interface Base<T> { [s: string]: Deep<T> }\nexport interface I<T> extends Base<T> {}\nexport declare const x: I<string>",
                        ),
                    ),
                    (
                        "use.ts",
                        "import { x } from \"./dep.js\"\nexport const y = x.foo".to_string(),
                    ),
                ],
            ),
            (
                "skipped declaration alias default",
                vec![
                    ("test.ts", r#"export * as ns from "./dep.js""#.to_string()),
                    (
                        "dep.d.ts",
                        p(
                            "type D<T = Deep<string>> = T\nexport interface X { v: D }\nexport declare const x: X",
                        ),
                    ),
                    ("use.ts", "import { x } from \"./dep.js\"\nexport const y = x.v".to_string()),
                ],
            ),
            (
                "compound indexed callable return",
                vec![
                    ("test.ts", r#"export * as ns from "./dep.js""#.to_string()),
                    (
                        "dep.d.ts",
                        p(
                            "export interface I<T> { v: Deep<T> }\ndeclare function make<T>(): () => T[\"v\" & keyof T]\nexport declare const x: ReturnType<typeof make<I<string>>>",
                        ),
                    ),
                    ("use.ts", "import { x } from \"./dep.js\"\nexport const y = x()".to_string()),
                ],
            ),
        ];

        #[derive(Default, Clone)]
        struct Outcome {
            diagnostics: Vec<String>,
            use_type: String,
        }
        for (name, files) in &cases {
            t.run(name, |t| {
                t.parallel();
                let files = borrow_files(files);
                // Rust: Go's `map[bool]...`, indexed by `enabled as usize`.
                let mut results: [Outcome; 2] = Default::default();
                let mut program_results: [Vec<String>; 2] = Default::default();
                for enabled in [false, true] {
                    let (program, mut c) = api_stability_program(t, &files, enabled);
                    let test_file = source_file(program, "/.src/test.ts");
                    let _ = c.get_diagnostics_exported(&Context::background(), test_file);
                    let use_file = source_file(program, "/.src/use.ts");
                    let diagnostics = api_stability_diagnostics(t, &mut c, use_file);
                    let y = api_stability_export(t, &mut c, use_file, "y");
                    let y_type = c.get_type_of_symbol_exported(y);
                    results[enabled as usize] =
                        Outcome { diagnostics, use_type: c.type_to_string_exported(y_type) };
                    drop(c);

                    program_results[enabled as usize] =
                        api_stability_whole_program_diagnostics(t, &files, enabled);
                }
                if results[0].diagnostics != results[1].diagnostics {
                    t.error(sprintf!(
                        "later diagnostics differ:\ndisabled=%v\nenabled=%v",
                        results[0].diagnostics,
                        results[1].diagnostics
                    ));
                }
                if results[0].use_type != results[1].use_type {
                    t.error(sprintf!(
                        "later type differs: disabled=%q enabled=%q",
                        results[0].use_type,
                        results[1].use_type
                    ));
                }
                if program_results[0] != program_results[1] {
                    t.error(sprintf!(
                        "whole-program diagnostics differ:\ndisabled=%v\nenabled=%v",
                        program_results[0],
                        program_results[1]
                    ));
                }
            });
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:495 TestApiStabilityLeakInferredForwardingDiagnostics 0ef3333b744caf80
#[test]
fn test_api_stability_leak_inferred_forwarding_diagnostics() {
    ets_go::testing::run(|t| {
        t.parallel();

        for expression in ["nope", "(1).missing", "(1)()"] {
            t.run(expression, |t| {
                t.parallel();
                let dep = format!("export function f() {{ return {expression} }}");
                let files =
                    [("test.ts", r#"export { f } from "./dep.js""#), ("dep.ts", dep.as_str())];
                let mut results: [Vec<String>; 2] = Default::default();
                let mut program_results: [Vec<String>; 2] = Default::default();
                for enabled in [false, true] {
                    let (program, mut c) = api_stability_program(t, &files, enabled);
                    let _ = c.get_diagnostics_exported(
                        &Context::background(),
                        source_file(program, "/.src/test.ts"),
                    );
                    results[enabled as usize] =
                        api_stability_diagnostics(t, &mut c, source_file(program, "/.src/dep.ts"));
                    drop(c);

                    program_results[enabled as usize] =
                        api_stability_whole_program_diagnostics(t, &files, enabled);
                }
                if results[0] != results[1] {
                    t.error(sprintf!(
                        "later dependency diagnostics differ:\ndisabled=%v\nenabled=%v",
                        results[0],
                        results[1]
                    ));
                }
                if program_results[0] != program_results[1] {
                    t.error(sprintf!(
                        "whole-program diagnostics differ:\ndisabled=%v\nenabled=%v",
                        program_results[0],
                        program_results[1]
                    ));
                }
                if results[0].is_empty() {
                    t.fatal(sprintf!(
                        "control reported no later dependency diagnostics for %q",
                        expression
                    ));
                }
            });
        }
    });
}

/// Rust-only: Go's `%v` of a `[]*ast.Diagnostic` in a failure message (Go prints the pointers); the messages are
/// printed instead.
fn diagnostic_messages(diagnostics: &[P<Diagnostic>]) -> Vec<String> {
    diagnostics.iter().map(|d| d.localize()).collect()
}

// go test: internal/rulerunner/api_stability_leak_test.go:530 TestApiStabilityLeakBlockedSurfaceNotCached 7a482d7f36623c69
#[test]
fn test_api_stability_leak_blocked_surface_not_cached() {
    ets_go::testing::run(|t| {
        t.parallel();

        let recursive = format!(
            "{API_STABILITY_RECURSIVE_PREFIX}export interface Base<T> {{ [s: string]: Deep<T> }}\nexport interface I<T> extends Base<T> {{}}\nexport declare const x: I<string>\n"
        );
        let cases = [
            (
                "finite inherited index",
                "type Step<T extends unknown[]> = T extends [unknown, ...infer Tail] ? Step<Tail> : number
export interface Base<T extends unknown[]> { [s: string]: Step<T> }
export interface I<T extends unknown[]> extends Base<T> {}
export declare const x: I<[0, 0, 0, 0, 0, 0, 0, 0, 0, 0]>
",
                "number",
            ),
            ("recursive inherited index", recursive.as_str(), "any"),
        ];
        for (name, source, want_value) in cases {
            t.run(name, |t| {
                t.parallel();
                let files = [("test.ts", source)];
                let (program, mut c) = api_stability_program(t, &files, false);
                let sf = source_file(program, "/.src/test.ts");
                let x = api_stability_export(t, &mut c, sf, "x");
                let mut tp = ets_typeparser::new_type_parser(program, &mut c);

                let mut first_delta: u32 = 0;
                let mut last: Option<ApiStabilityUsed> = None;
                for i in 0..5_i64 {
                    let before = tp.checker().total_instantiation_count;
                    last = Some(tp.api_stability_used_by_symbol(x));
                    let delta = tp.checker().total_instantiation_count - before;
                    if i == 0 {
                        first_delta = delta;
                        continue;
                    }
                    if delta != 0 {
                        t.error(sprintf!("query %d repeated %d instantiations, want 0", i, delta));
                    }
                }
                if !last.is_some_and(|last| last.incomplete()) {
                    t.error("a blocked recursive surface must report incomplete, not stable");
                }
                if first_delta > 64 {
                    t.error(sprintf!(
                        "first query instantiated %d types, want a bounded pre-flight",
                        first_delta
                    ));
                }
                let c = tp.checker();
                let x_symbol = api_stability_export(t, c, sf, "x");
                let value = c.get_type_of_symbol_exported(x_symbol);
                let infos = c.get_index_infos_of_type_exported(value);
                if infos.is_empty() {
                    let globals = diagnostic_messages(&c.get_global_diagnostics());
                    t.fatal(sprintf!("ordinary index resolution lost its index info: %v", globals));
                }
                let got = c.type_to_string_exported(infos[0].value_type());
                if got != want_value {
                    let globals = diagnostic_messages(&c.get_global_diagnostics());
                    t.error(sprintf!(
                        "ordinary index value=%q, want %q (globals=%v)",
                        got,
                        want_value,
                        globals
                    ));
                }
                if name == "finite inherited index" && !c.get_global_diagnostics().is_empty() {
                    let globals = diagnostic_messages(&c.get_global_diagnostics());
                    t.error(sprintf!(
                        "ordinary finite read produced global diagnostics: %v",
                        globals
                    ));
                }
            });
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:601 TestApiStabilityLeakRelationSafety b251723244c045cf
#[test]
fn test_api_stability_leak_relation_safety() {
    ets_go::testing::run(|t| {
        t.parallel();
        let a = |s: &str| format!("{API_STABILITY_RECURSIVE_PREFIX}{s}");

        let cases = [
            (
                "callable relation",
                a(
                    "interface F<T> { (): Deep<T> }\ntype Select<T> = T extends (() => number) ? string : number\nexport interface X { v: Select<F<string>> }\nexport declare const x: X",
                ),
                "x.v",
            ),
            (
                "property relation",
                a(
                    "interface F<T> { v: Deep<T> }\ntype Select<T> = T extends { v: number } ? string : number\nexport interface X { v: Select<F<string>> }\nexport declare const x: X",
                ),
                "x.v",
            ),
            (
                "array conditional substitution",
                a(
                    "type Select<T> = T[] extends string[] ? Deep<string> : number\nexport interface X { v: Select<string> }\nexport declare const x: X",
                ),
                "x.v",
            ),
            (
                "reference conditional substitution",
                a(
                    "interface Box<T> { value: T }\ntype Select<T> = Box<T> extends Box<string> ? Deep<string> : number\nexport interface X { v: Select<string> }\nexport declare const x: X",
                ),
                "x.v",
            ),
            (
                "never conditional",
                a(
                    "type Select = never extends string ? Deep<string> : number\nexport interface X { v: Select }\nexport declare const x: X",
                ),
                "x.v",
            ),
            (
                "ignored explicit alias argument",
                a(
                    "type Ignore<T> = number\nexport interface X { v: Ignore<Deep<string>> }\nexport declare const x: X",
                ),
                "x.v",
            ),
            (
                "inherited projection",
                a(
                    "interface Base<T> { v: Deep<T> }\ninterface F<T> extends Base<T> {}\ntype Read<T> = T[\"v\" & keyof T]\nexport interface X { v: Read<F<string>> }\nexport declare const x: X",
                ),
                "x.v",
            ),
        ];

        #[derive(Default, Clone, PartialEq)]
        struct Outcome {
            pre: Vec<String>,
            later: Vec<String>,
            use_type: String,
            globals: i64,
        }
        ets_go::impl_go_struct!(Outcome = "rulerunner_test.outcome" {
            "pre": pre, "later": later, "useType": use_type, "globals": globals,
        });
        for (name, dep, expr) in &cases {
            t.run(name, |t| {
                t.parallel();
                let use_ts = format!("import {{ x }} from \"./dep.js\"\nexport const y = {expr}");
                let files = [
                    ("test.ts", r#"export * as ns from "./dep.js""#),
                    ("dep.d.ts", dep.as_str()),
                    ("use.ts", use_ts.as_str()),
                ];
                let mut results: [Outcome; 2] = Default::default();
                let mut program_results: [Vec<String>; 2] = Default::default();
                for enabled in [false, true] {
                    let (program, mut c) = api_stability_program(t, &files, enabled);
                    let test_file = source_file(program, "/.src/test.ts");
                    let before = c.total_instantiation_count;
                    let pre = api_stability_diagnostics(t, &mut c, test_file);
                    let delta = c.total_instantiation_count - before;
                    let dep_file = source_file(program, "/.src/dep.d.ts");
                    if enabled {
                        let x = api_stability_export(t, &mut c, dep_file, "X");
                        let refused =
                            ets_typeparser::new_type_parser(program, &mut c).api_stability_used_by_symbol(x);
                        if !refused.incomplete() {
                            t.error("a refused recursive surface must report incomplete before the dependency is checked");
                        }
                    }
                    let use_file = source_file(program, "/.src/use.ts");
                    let later = api_stability_diagnostics(t, &mut c, use_file);
                    let y = api_stability_export(t, &mut c, use_file, "y");
                    let y_type = c.get_type_of_symbol_exported(y);
                    let use_type = c.type_to_string_exported(y_type);
                    let globals = c.get_global_diagnostics().len() as i64;
                    results[enabled as usize] = Outcome { pre, later, use_type, globals };
                    let result = &results[enabled as usize];
                    t.log(sprintf!(
                        "enabled=%v delta=%d pre=%v later=%v type=%s globals=%d",
                        enabled,
                        delta,
                        result.pre,
                        result.later,
                        result.use_type,
                        result.globals
                    ));
                    if enabled && delta > 256 {
                        t.error(sprintf!(
                            "rule instantiated %d types before blocking, want at most 256",
                            delta
                        ));
                    }
                    drop(c);

                    program_results[enabled as usize] =
                        api_stability_whole_program_diagnostics(t, &files, enabled);
                }
                if results[0] != results[1] {
                    t.error(sprintf!(
                        "rule changed checker behavior:\ndisabled=%+v\nenabled=%+v",
                        results[0],
                        results[1]
                    ));
                }
                if program_results[0] != program_results[1] {
                    t.error(sprintf!(
                        "rule changed whole-program diagnostics:\ndisabled=%v\nenabled=%v",
                        program_results[0],
                        program_results[1]
                    ));
                }
            });
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:725 TestApiStabilityLeakRefusedSurfaceStaysRetryable 571ac330a0100afe
#[test]
fn test_api_stability_leak_refused_surface_stays_retryable() {
    ets_go::testing::run(|t| {
        t.parallel();
        let a = |s: &str| format!("{API_STABILITY_RECURSIVE_PREFIX}{s}");

        let cases = [
            (
                "callable relation",
                a(
                    "interface F<T> { (): Deep<T> }\ntype Select<T> = T extends (() => number) ? string : number\nexport interface X { v: Select<F<string>> }\nexport declare const x: X",
                ),
            ),
            (
                "inherited projection",
                a(
                    "interface Base<T> { v: Deep<T> }\ninterface F<T> extends Base<T> {}\ntype Read<T> = T[\"v\" & keyof T]\nexport interface X { v: Read<F<string>> }\nexport declare const x: X",
                ),
            ),
            (
                "ignored explicit alias argument",
                a(
                    "type Ignore<T> = number\nexport interface X { v: Ignore<Deep<string>> }\nexport declare const x: X",
                ),
            ),
        ];
        for (name, dep) in &cases {
            t.run(name, |t| {
                t.parallel();
                let files =
                    [("dep.d.ts", dep.as_str()), ("test.ts", r#"export * as ns from "./dep.js""#)];
                let (program, mut c) = api_stability_program(t, &files, false);
                let _ = c.get_diagnostics_exported(
                    &Context::background(),
                    source_file(program, "/.src/test.ts"),
                );
                let symbol =
                    api_stability_export(t, &mut c, source_file(program, "/.src/dep.d.ts"), "X");
                let mut tp = ets_typeparser::new_type_parser(program, &mut c);
                for i in 0..5_i64 {
                    let before = tp.checker().total_instantiation_count;
                    let used = tp.api_stability_used_by_symbol(symbol);
                    let delta = tp.checker().total_instantiation_count - before;
                    if !used.incomplete() {
                        t.fatal(sprintf!("query %d reported a refused surface as stable", i));
                    }
                    if i > 0 && delta != 0 {
                        t.error(sprintf!("query %d repeated %d instantiations, want 0", i, delta));
                    }
                }
            });
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:766 TestApiStabilityLeakInferredDiagnosticsStayExact dedf17e80decbfef
#[test]
fn test_api_stability_leak_inferred_diagnostics_stay_exact() {
    ets_go::testing::run(|t| {
        t.parallel();
        let a = |s: &str| format!("{API_STABILITY_RECURSIVE_PREFIX}{s}");

        let cases = [
            ("name error body", "export function f() { return nope }".to_string()),
            (
                "recursive body",
                a("declare const deep: Deep<string>\nexport function f() { return deep }"),
            ),
            (
                "recursive initializer",
                a("declare const deep: Deep<string>\nexport const f = { nested: { deep } }"),
            ),
            ("bad initializer", "export const f = nope".to_string()),
        ];
        for (name, dep) in &cases {
            t.run(name, |t| {
                t.parallel();
                let files =
                    [("test.ts", r#"export { f } from "./dep.js""#), ("dep.ts", dep.as_str())];
                let mut file_results: [Vec<String>; 2] = Default::default();
                let mut program_results: [Vec<String>; 2] = Default::default();
                for enabled in [false, true] {
                    let (program, mut c) = api_stability_program(t, &files, enabled);
                    let mut pre =
                        api_stability_diagnostics(t, &mut c, source_file(program, "/.src/test.ts"));
                    let later =
                        api_stability_diagnostics(t, &mut c, source_file(program, "/.src/dep.ts"));
                    pre.extend(later);
                    file_results[enabled as usize] = pre;
                    drop(c);

                    program_results[enabled as usize] =
                        api_stability_whole_program_diagnostics(t, &files, enabled);
                }
                if file_results[0] != file_results[1] {
                    t.error(sprintf!(
                        "dependency diagnostics differ:\ndisabled=%v\nenabled=%v",
                        file_results[0],
                        file_results[1]
                    ));
                }
                if program_results[0] != program_results[1] {
                    t.error(sprintf!(
                        "whole-program diagnostics differ:\ndisabled=%v\nenabled=%v",
                        program_results[0],
                        program_results[1]
                    ));
                }
            });
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:804 TestApiStabilityLeakRelationOperands e847c523eb6eb342
#[test]
fn test_api_stability_leak_relation_operands() {
    ets_go::testing::run(|t| {
        t.parallel();

        // Rust: (name, dep, ts).
        let cases = [
            (
                "callable return",
                "interface G<T> { v: Deep<T> }
interface F<T> { (): G<T> }
type Select<T> = T extends (() => { v: number }) ? string : number
export interface X { v: Select<F<string>> }",
                false,
            ),
            (
                "callable parameter",
                "interface G<T> { v: Deep<T> }
interface F<T> { (x: G<T>): void }
type Select<T> = T extends ((x: { v: number }) => void) ? string : number
export interface X { v: Select<F<string>> }",
                false,
            ),
            (
                "construct return",
                "interface G<T> { v: Deep<T> }
interface F<T> { new (): G<T> }
type Select<T> = T extends (new () => { v: number }) ? string : number
export interface X { v: Select<F<string>> }",
                false,
            ),
            (
                "method return",
                "interface G<T> { v: Deep<T> }
interface F<T> { m(): G<T> }
type Select<T> = T extends { m(): { v: number } } ? string : number
export interface X { v: Select<F<string>> }",
                false,
            ),
            (
                "inherited callable return",
                "interface G<T> { v: Deep<T> }
interface B<T> { (): G<T> }
interface F<T> extends B<T> {}
type Select<T> = T extends (() => { v: number }) ? string : number
export interface X { v: Select<F<string>> }",
                false,
            ),
            (
                "anonymous object compound",
                "type Select<T> = { v: T } extends { v: string } ? Deep<string> : number
export interface X { v: Select<string> }",
                false,
            ),
            (
                "anonymous function compound",
                "type Select<T> = (() => T) extends (() => string) ? Deep<string> : number
export interface X { v: Select<string> }",
                false,
            ),
            (
                "anonymous method compound",
                "type Select<T> = { m(): T } extends { m(): string } ? Deep<string> : number
export interface X { v: Select<string> }",
                false,
            ),
            (
                "mapped compound",
                "type Select<T> = { [K in keyof T]: Deep<K> } extends { toString(): string } ? number : Deep<string>
export interface X { v: Select<string> }",
                false,
            ),
            (
                "generic callable constraint",
                "interface G<T> { v: Deep<T> }
interface F<T> { <U extends G<T>>(x: U): void }
type Select<T> = T extends (<U extends { v: number }>(x: U) => void) ? Deep<string> : number
export interface X { v: Select<F<string>> }",
                false,
            ),
            (
                "generic callable default",
                "interface F<T> { <U = Deep<T>>(): U }
type Select<T> = T extends (() => number) ? Deep<string> : number
export interface X { v: Select<F<string>> }",
                false,
            ),
            (
                "inferred class property relation",
                "declare function make<T>(): Deep<T>
class F { v = make<string>() }
type Select<T> = T extends { v: number } ? string : number
export interface X { y: Select<F> }",
                true,
            ),
        ];

        for (name, dep, ts) in cases {
            t.run(name, |t| {
                t.parallel();
                let dep_name = if ts { "dep.ts" } else { "dep.d.ts" };
                let dep_source = format!("{API_STABILITY_RECURSIVE_PREFIX}{dep}");
                let files = [
                    (dep_name, dep_source.as_str()),
                    ("test.ts", r#"export * as ns from "./dep.js""#),
                    ("use.ts", "import { X } from \"./dep.js\"\nexport declare const z: X"),
                ];
                let mut file_results: [Vec<String>; 2] = Default::default();
                let mut program_results: [Vec<String>; 2] = Default::default();
                for enabled in [false, true] {
                    let (program, mut c) = api_stability_program(t, &files, enabled);
                    let test_file = source_file(program, "/.src/test.ts");
                    let use_file = source_file(program, "/.src/use.ts");
                    let dep_file = source_file(program, &format!("/.src/{dep_name}"));
                    let mut collected = Vec::new();
                    collected.extend(api_stability_diagnostics(t, &mut c, test_file));
                    collected.extend(api_stability_diagnostics(t, &mut c, use_file));
                    if enabled {
                        let x = api_stability_export(t, &mut c, dep_file, "X");
                        let refused =
                            ets_typeparser::new_type_parser(program, &mut c).api_stability_used_by_symbol(x);
                        if !refused.incomplete() {
                            t.error("refused surface X was not reported incomplete before the dependency was checked");
                        }
                    }
                    let first = api_stability_diagnostics(t, &mut c, dep_file);
                    let second = api_stability_diagnostics(t, &mut c, dep_file);
                    if first != second {
                        t.error(sprintf!(
                            "repeated dependency diagnostics differ:\nfirst=%v\nsecond=%v",
                            first,
                            second
                        ));
                    }
                    collected.extend(first);
                    file_results[enabled as usize] = collected;
                    drop(c);

                    program_results[enabled as usize] =
                        api_stability_whole_program_diagnostics(t, &files, enabled);
                }
                if file_results[0] != file_results[1] {
                    t.error(sprintf!(
                        "rule changed checker behavior:\ndisabled=%v\nenabled=%v",
                        file_results[0],
                        file_results[1]
                    ));
                }
                if program_results[0] != program_results[1] {
                    t.error(sprintf!(
                        "rule changed whole-program diagnostics:\ndisabled=%v\nenabled=%v",
                        program_results[0],
                        program_results[1]
                    ));
                }
            });
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:942 apiStabilityAccessorDiagnostic 8ddc0a752ca1597a
fn api_stability_accessor_diagnostic(diagnostic: P<Diagnostic>) -> String {
    let mut name = "global".to_string();
    if let Some(file) = diagnostic.file() {
        name = file.file_name().to_string();
    }
    format!("{name}:{}", diagnostic.localize())
}

// go test: internal/rulerunner/api_stability_leak_test.go:950 TestApiStabilityLeakAccessorDiagnosticsExact 197b166d6e42c8c0
#[test]
fn test_api_stability_leak_accessor_diagnostics_exact() {
    ets_go::testing::run(|t| {
        t.parallel();

        const PREFIX: &str =
            "/** @stability experimental */\ninterface E { v: string }\ndeclare const e: E\n";
        let p = |s: &str| format!("{PREFIX}{s}");
        let deps = [
            p("export const f = { get v() { return e } }"),
            p("export const f = { get v() { return nope } }"),
            p(r#"export const f = { get v() { const x: number = "s"; return x } }"#),
            p("export const f = { set v(x: number) { x.missing } }"),
            p("export const f = { get v(): E { return nope } }"),
            p("export class C { get value() { return nope } }"),
            p(
                "function make<T>(t: T) { return { get v() { return t } } }\nexport const f = make(e)",
            ),
        ];
        for dep in &deps {
            t.run(dep, |t| {
                t.parallel();

                let files =
                    [("test.ts", r#"export { f } from "./dep.js""#), ("dep.ts", dep.as_str())];
                let mut out: [Vec<String>; 2] = Default::default();
                for enabled in [false, true] {
                    let (program, mut c) = api_stability_program(t, &files, enabled);
                    let mut collected = Vec::new();
                    for file in ["/.src/test.ts", "/.src/dep.ts", "/.src/dep.ts"] {
                        for diagnostic in c.get_diagnostics_exported(
                            &Context::background(),
                            source_file(program, file),
                        ) {
                            if is_rule_message(&diagnostic.localize()) {
                                continue;
                            }
                            collected.push(api_stability_accessor_diagnostic(diagnostic));
                        }
                        for diagnostic in c.get_global_diagnostics() {
                            collected.push(api_stability_accessor_diagnostic(diagnostic));
                        }
                    }
                    out[enabled as usize] = collected;
                    drop(c);
                }
                if out[0] != out[1] {
                    t.error(sprintf!(
                        "accessor read changed diagnostics:\ndisabled=%v\nenabled=%v",
                        out[0].join("\n"),
                        out[1].join("\n")
                    ));
                }
            });
        }
    });
}
