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

// go test: internal/rulerunner/api_stability_leak_test.go:30 apiStabilityLifecycleOptions
// apiStabilityLifecycleOptions builds the effect plugin options with only the
// API stability leak rule enabled at warning severity (or disabled entirely
// for the control run).
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

// go test: internal/rulerunner/api_stability_leak_test.go:49 apiStabilityLifecycleDiagnostics
// apiStabilityLifecycleDiagnostics runs a virtual multi-file project through
// the real after-check callback path (etscheckerhooks -> rulerunner) and returns
// the diagnostics reported for the project, including global (file-less)
// diagnostics such as TS2589. With wholeProgram set it asks the program for the
// semantic diagnostics; otherwise it asks the checker for the file diagnostics,
// which is the lifecycle the language service uses. Globals are collected on
// both paths: a rule-triggered instantiation guard reports TS2589 as a global,
// and omitting globals would hide it from the enabled/disabled comparison.
//
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

// go test: internal/rulerunner/api_stability_leak_test.go:114 apiStabilityLifecycleContains
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

/// Rust-only: Go's `files[name]` on a test's file map (`""` when absent).
fn file_source<'a>(files: &[(&str, &'a str)], name: &str) -> &'a str {
    files.iter().find(|(n, _)| *n == name).map_or("", |(_, source)| *source)
}

/// Rust-only: Go's `strings.Contains(message, "effect(apiStabilityLeak)")`, repeated by every case.
fn is_rule_message(message: &str) -> bool {
    message.contains("effect(apiStabilityLeak)")
}

/// Rust-only: a lifecycle table case (Go's anonymous `struct{name; files; want; notWant}`).
struct LifecycleCase {
    name: &'static str,
    files: Vec<(&'static str, String)>,
    want: Vec<&'static str>,
    not_want: Vec<&'static str>,
}

/// Rust-only: builds a [`LifecycleCase`].
fn lifecycle_case(
    name: &'static str,
    files: Vec<(&'static str, String)>,
    want: &[&'static str],
    not_want: &[&'static str],
) -> LifecycleCase {
    LifecycleCase { name, files, want: want.to_vec(), not_want: not_want.to_vec() }
}

// go test: internal/rulerunner/api_stability_leak_test.go:128 TestApiStabilityLeakLifecycle
// apiStabilityLifecycleCases covers the lazy-read lifecycle findings under the
// corrected boundary. Each case runs through the production after-check
// callback with normal diagnostics, in both the per-file and whole-program
// lifecycles, and compares against a control run with the rule disabled so a
// compiler diagnostic introduced by the rule cannot hide.
#[test]
#[expect(clippy::literal_string_with_formatting_args, reason = "TypeScript fixtures with braces")]
fn test_api_stability_leak_lifecycle() {
    ets_go::testing::run(|t| {
        t.parallel();
        let prefix =
            "/** @stability experimental */\ninterface E {value:string}\ndeclare const e:E\n";
        let p = |s: &str| format!("{prefix}{s}");

        let cases = vec![
            lifecycle_case(
                "anonymous inferred conditional return",
                vec![("test.ts", p("declare function make<T>():{value:T extends string ? number:E}; export const x=make<number>()"))],
                &["`x` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "inferred callable conditional return",
                vec![("test.ts", p("declare function make<T>():()=>T extends string ? number:E; export const x=make<number>()"))],
                &["`x` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "nested anonymous conditional return",
                vec![("test.ts", p("declare function make<T>():{f:()=>{value:T extends string?number:E}}; export const x=make<number>()"))],
                &["`x` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "contravariant conditional member",
                vec![("test.ts", p(r#"interface C<in T>{f:(x:T)=>void}; interface Holder<T>{value:T extends C<string> ? number:E}; export declare const x:Holder<C<"x">>"#))],
                &["`x` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "invariant conditional member",
                vec![("test.ts", p(r#"interface C<in out T>{f:(x:T)=>T}; interface Holder<T>{value:T extends C<string> ? number:E}; export declare const x:Holder<C<"x">>"#))],
                &["`x` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "array wrapped conditional member",
                vec![("test.ts", p(r#"interface C<in T>{f:(x:T)=>void}; interface Holder<T>{value:T extends C<string>[] ? number:E}; export declare const x:Holder<C<"x">[]>"#))],
                &["`x` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "structural conditional member stays clean",
                vec![("test.ts", p("interface Holder<T>{value:T extends {value:string} ? number:E}; export declare const x:Holder<{value:string}>"))],
                &[],
                &["`x` exposes `E`"],
            ),
            lifecycle_case(
                "represented indexed callable return",
                vec![("test.ts", p(r#"interface I{value:E}; type Read<T>=T["value" & keyof T]; declare function make<T>():()=>Read<T>; export const x=make<I>(); x()"#))],
                &["`x` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "lazily materialized indexed callable return",
                vec![("test.ts", p(r#"interface I{value:E}; type Read<T>=T["value" & keyof T]; declare function make<T>():()=>Read<T>; export const x=make<I>()"#))],
                &["`x` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "single-file barrel ts dependency",
                vec![
                    ("test.ts", r#"export {f} from "./dep.js""#.to_string()),
                    ("dep.ts", p("export declare function f(x:E):void")),
                ],
                &["`f` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "single-file barrel dts dependency",
                vec![
                    ("test.ts", r#"export {f} from "./dep.js""#.to_string()),
                    ("dep.d.ts", p("export declare function f(x:E):void")),
                ],
                &["`f` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "single-file star barrel dts dependency",
                vec![
                    ("test.ts", r#"export * from "./dep.js""#.to_string()),
                    ("dep.d.ts", p("export interface X {value:E}")),
                ],
                &["`X` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "shallow nested named reference stays quiet",
                vec![("test.ts", p("interface Hidden<T>{value:E}; declare const h:Hidden<string>; h.value; export interface X{hidden:Hidden<string>}"))],
                &[],
                &["`X` exposes `E`"],
            ),
            lifecycle_case(
                "shallow nested tagged reference stays quiet",
                vec![("test.ts", p("interface Hidden<T>{/** @stability experimental */ value:string}; declare const h:Hidden<string>; h.value; export interface X{hidden:Hidden<string>}"))],
                &[],
                &["`X` exposes `E`"],
            ),
            lifecycle_case(
                "erased alias in collapsed union",
                vec![("test.ts", "/** @stability experimental */\ntype A=string\nexport declare const x: never|number|A".to_string())],
                &["`x` exposes `A`"],
                &[],
            ),
            lifecycle_case(
                "erased alias in reordered union",
                vec![("test.ts", "/** @stability experimental */\ntype A=string\nexport declare const x: number|A|never".to_string())],
                &["`x` exposes `A`"],
                &[],
            ),
            lifecycle_case(
                "erased alias in collapsed intersection",
                vec![("test.ts", "/** @stability experimental */\ntype A=string\nexport declare const x: A&string".to_string())],
                &["`x` exposes `A`"],
                &[],
            ),
            lifecycle_case(
                "declared conditional alias branch",
                vec![("test.ts", p("export type X<T> = T extends string ? number : E"))],
                &["`X` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "finite recursive alias application",
                vec![("test.ts", p("type A<T> = T extends string ? A<number> : E\nexport declare const x: A<string>"))],
                &["`x` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "finite recursive tuple alias application",
                vec![("test.ts", p("type Step<T extends unknown[]> = T extends [unknown, ...infer Tail] ? Step<Tail> : E\nexport declare const x: Step<[0, 0]>"))],
                &["`x` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "declared index signature tag",
                vec![("test.ts", "export interface I {\n/** @stability experimental */\n[key:string]:number\n}".to_string())],
                &["`I` exposes `index signature`"],
                &[],
            ),
        ];

        for test in &cases {
            for whole_program in [false, true] {
                let mode = if whole_program { "program" } else { "file" };
                t.run(&format!("{}/{mode}", test.name), |t| {
                    t.parallel();
                    let files = borrow_files(&test.files);
                    let messages =
                        api_stability_lifecycle_diagnostics(t, &files, true, whole_program);
                    let control =
                        api_stability_lifecycle_diagnostics(t, &files, false, whole_program);
                    for want in &test.want {
                        if !api_stability_lifecycle_contains(&messages, want) {
                            t.error(sprintf!("missing %q in %v", want, messages));
                        }
                    }
                    for not_want in &test.not_want {
                        if api_stability_lifecycle_contains(&messages, not_want) {
                            t.error(sprintf!("unexpected %q in %v", not_want, messages));
                        }
                    }
                    // The rule must not introduce compiler diagnostics on
                    // semantically valid code: every non-rule message must also be
                    // present in the control. Rule messages carry the effect code.
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
                });
            }
        }
    });
}

/// Rust-only: the case loop shared by the export-visibility and import-equals lifecycle tests (Go repeats it in
/// both).
fn run_strict_lifecycle_cases(t: &T, cases: &[LifecycleCase]) {
    for test in cases {
        for whole_program in [false, true] {
            let mode = if whole_program { "program" } else { "file" };
            t.run(&format!("{}/{mode}", test.name), |t| {
                t.parallel();
                let files = borrow_files(&test.files);
                let control = api_stability_lifecycle_diagnostics(t, &files, false, whole_program);
                if !control.is_empty() {
                    t.fatal(sprintf!("invalid control: %v", control));
                }
                let messages = api_stability_lifecycle_diagnostics(t, &files, true, whole_program);
                for want in &test.want {
                    if !api_stability_lifecycle_contains(&messages, want) {
                        t.error(sprintf!("missing %q in %v", want, messages));
                    }
                }
                for not_want in &test.not_want {
                    if api_stability_lifecycle_contains(&messages, not_want) {
                        t.error(sprintf!("unexpected %q in %v", not_want, messages));
                    }
                }
                for message in &messages {
                    if is_rule_message(message) {
                        continue;
                    }
                    t.error(sprintf!(
                        "rule introduced compiler diagnostic %q\nrule=%v\ncontrol=%v",
                        message,
                        messages,
                        control
                    ));
                }
                if messages.join("\n").contains("TS2589") {
                    t.error(sprintf!("rule introduced TS2589: %v", messages));
                }
            });
        }
    }
}

// go test: internal/rulerunner/api_stability_leak_test.go:299 TestApiStabilityLeakExportVisibilityLifecycle
// TestApiStabilityLeakExportVisibilityLifecycle runs the corrected
// export-level visibility policy through the production after-check callback in
// both lifecycles: the explicit export winner beats a redundant star forward,
// declared namespaces filter tagged members, and import-then-export aliases
// follow their import target. Public peers and public targets still warn, and
// the disabled-rule control stays diagnostic-free, so no compiler diagnostic is
// introduced by the metadata decisions.
#[test]
fn test_api_stability_leak_export_visibility_lifecycle() {
    ets_go::testing::run(|t| {
        t.parallel();
        let prefix = "/** @stability experimental */\ninterface E { value: string }\n/** @stability experimental */\ninterface F { value: string }\n";
        let p = |s: &str| format!("{prefix}{s}");

        let cases = vec![
            lifecycle_case(
                "tagged named export plus redundant star",
                vec![
                    ("test.ts", "/** @internal */\nexport { Leaky } from \"./dep\"\nexport * from \"./dep\"\n".to_string()),
                    ("dep.ts", p("export interface Leaky { value: E }\n")),
                ],
                &[],
                // dep.ts's own direct export of Leaky is public there and is
                // legitimately checked; only the tagged test.ts re-export is
                // expected to stay quiet.
                &["/.src/test.ts:`Leaky` exposes"],
            ),
            lifecycle_case(
                "tagged local export plus redundant cyclic star",
                vec![
                    ("test.ts", p("/** @internal */\nexport interface Leaky { value: E }\nexport * from \"./dep\"\n")),
                    ("dep.ts", "export { Leaky } from \"./test\"\n".to_string()),
                ],
                &[],
                &["`Leaky` exposes"],
            ),
            lifecycle_case(
                "public local export plus tagged redundant star stays checked",
                vec![
                    ("test.ts", p("export interface Leaky { value: E }\n/** @internal */\nexport * from \"./dep\"\n")),
                    ("dep.ts", "export * from \"./test\"\n".to_string()),
                ],
                &["`Leaky` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "declared namespace filters tagged member",
                vec![("test.ts", p("export namespace Api {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: string }\n}\n"))],
                &[],
                &["`Api` exposes `E`"],
            ),
            lifecycle_case(
                "declared namespace public peer still checked",
                vec![("test.ts", p("export namespace Api {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: F }\n}\n"))],
                &["`Api` exposes `F`"],
                &["`Api` exposes `E`"],
            ),
            lifecycle_case(
                "import then export follows internal named target",
                vec![
                    ("test.ts", "import { Hidden } from \"./dep\"\nexport { Hidden }\n".to_string()),
                    ("dep.ts", p("/** @internal */\nexport interface Hidden { value: E }\n")),
                ],
                &[],
                &["`Hidden` exposes"],
            ),
            lifecycle_case(
                "default import then export default follows internal target",
                vec![
                    ("test.ts", "import Hidden from \"./dep\"\nexport default Hidden\n".to_string()),
                    ("dep.ts", p("/** @internal */\nexport default function hidden(): E { return null as unknown as E }\n")),
                ],
                &[],
                &["`default` exposes"],
            ),
            lifecycle_case(
                "public import then export still warns",
                vec![
                    ("test.ts", "import { Shown } from \"./dep\"\nexport { Shown }\n".to_string()),
                    ("dep.ts", p("export interface Shown { value: E }\n")),
                ],
                &["`Shown` exposes `E`"],
                &[],
            ),
            lifecycle_case(
                "internal and public peers keep independent verdicts",
                vec![("test.ts", p("/** @internal */\nexport declare const aHidden: E\nexport declare const zShown: E\n"))],
                &["`zShown` exposes `E`"],
                &["`aHidden` exposes"],
            ),
        ];

        run_strict_lifecycle_cases(t, &cases);
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:429 TestApiStabilityLeakImportEqualsLifecycle
// TestApiStabilityLeakImportEqualsLifecycle pins the import-equals metadata
// decision in the production after-check lifecycle: an `export import X = ...` alias follows
// the target named by its module reference, so the tagged target stays quiet in
// both the file and whole-program hook paths while its untagged peer is checked,
// and the disabled-rule control stays diagnostic-free. The same decision is
// exercised for a namespace qualifier, a named import of that namespace and an
// external `require`.
#[test]
fn test_api_stability_leak_import_equals_lifecycle() {
    ets_go::testing::run(|t| {
        t.parallel();
        let prefix = "/** @stability experimental */\ninterface E { value: string }\n";
        let p = |s: &str| format!("{prefix}{s}");

        let cases = vec![
            lifecycle_case(
                "local namespace import equals follows internal target",
                vec![("test.ts", p("namespace Impl {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: E }\n}\nexport import Hidden = Impl.Hidden\nexport import Shown = Impl.Shown\n"))],
                &["`Shown` exposes `E`"],
                &["`Hidden` exposes"],
            ),
            lifecycle_case(
                "cross file namespace import equals follows internal target",
                vec![
                    ("test.ts", "import { Impl } from \"./dep\"\nexport import Hidden = Impl.Hidden\nexport import Shown = Impl.Shown\n".to_string()),
                    ("dep.ts", p("export namespace Impl {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: E }\n}\n")),
                ],
                &["`Shown` exposes `E`"],
                &["`Hidden` exposes"],
            ),
            lifecycle_case(
                "external require import equals follows internal member",
                vec![
                    ("test.ts", "import Dep = require(\"./dep\")\nexport import Hidden = Dep.Hidden\nexport import Shown = Dep.Shown\n".to_string()),
                    ("dep.ts", p("/** @internal */\nexport interface Hidden { value: E }\nexport interface Shown { value: E }\n")),
                ],
                &["`Shown` exposes `E`"],
                &["`Hidden` exposes"],
            ),
        ];

        run_strict_lifecycle_cases(t, &cases);
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:509 TestApiStabilityLeakVisibilityLifecycleRecursiveSafety
// TestApiStabilityLeakVisibilityLifecycleRecursiveSafety keeps the
// recursive-alias budget of the visibility fixtures: the star/cycle and
// namespace shapes used by the regressions must add no compiler
// diagnostics to a program whose checking is unaffected, including the repeated
// star cycle.
#[test]
fn test_api_stability_leak_visibility_lifecycle_recursive_safety() {
    ets_go::testing::run(|t| {
        t.parallel();
        let deep_alias = "type Deep<T> = T extends string ? Deep<T> : T\n";

        let test_ts = format!(
            "{deep_alias}/** @internal */\nexport interface Cyclic {{ value: Deep<string> }}\nexport * from \"./dep\"\n"
        );
        let files = [("test.ts", test_ts.as_str()), ("dep.ts", "export * from \"./test\"\n")];
        for whole_program in [false, true] {
            let mode = if whole_program { "program" } else { "file" };
            t.run(mode, |t| {
                t.parallel();
                let messages = api_stability_lifecycle_diagnostics(t, &files, true, whole_program);
                let control = api_stability_lifecycle_diagnostics(t, &files, false, whole_program);
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
                if api_stability_lifecycle_contains(&messages, "`Cyclic` exposes") {
                    t.error(sprintf!("tagged cyclic export was checked: %v", messages));
                }
            });
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:546 TestApiStabilityLeakLifecycleRecursiveSafety
// TestApiStabilityLeakLifecycleRecursiveSafety keeps the recursive-alias safety
// property in the production lifecycle: a recursive generic alias is never
// expanded by the rule, so the rule adds no TS2589 to a program whose own
// checking does not produce one, and a program that already produces one keeps
// exactly its baseline diagnostics.
#[test]
fn test_api_stability_leak_lifecycle_recursive_safety() {
    ets_go::testing::run(|t| {
        t.parallel();
        let deep_alias = "type Deep<T> = T extends string ? Deep<T> : T\n";

        t.run("member budget adds no diagnostic", |t| {
            t.parallel();
            let source = format!(
                "{deep_alias}interface Wrapper<T> {{ value: Deep<T> }}\nexport declare const exposed: Wrapper<string>\n"
            );
            let files = [("test.ts", source.as_str())];
            let messages = api_stability_lifecycle_diagnostics(t, &files, true, true);
            let control = api_stability_lifecycle_diagnostics(t, &files, false, true);
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
        });

        t.run("computed name baseline preserved", |t| {
            t.parallel();
            let source = format!(
                "{deep_alias}declare const key: Deep<string>\nexport interface X {{ [key]: number }}\n"
            );
            let files = [("test.ts", source.as_str())];
            let messages = api_stability_lifecycle_diagnostics(t, &files, true, true);
            let control = api_stability_lifecycle_diagnostics(t, &files, false, true);
            // The baseline already reports TS2589 for this intentionally invalid
            // input; the rule must not change that diagnostic set.
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
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:588 apiStabilityRecursivePrefix
// apiStabilityRecursivePrefix declares the recursive alias used by the recursive-alias
// safety regressions. `Deep<T>` keeps expanding itself so any speculative
// resolution of an application of it would trip the checker's depth guard.
const API_STABILITY_RECURSIVE_PREFIX: &str = "type Deep<T> = T extends string ? Deep<T> : T\n";

// go test: internal/rulerunner/api_stability_leak_test.go:596 runApiStabilitySafetyCase
// runApiStabilitySafetyCase runs one project through the production after-check
// callback in the per-file and whole-program lifecycles, with the rule enabled
// and disabled. It asserts that the rule changes no compiler diagnostic on
// either lifecycle in either direction (added or lost, globals included, so a
// rule-provoked TS2589 cannot hide and a suppressed ordinary diagnostic cannot
// disappear) and checks the expected and forbidden rule diagnostics.
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

// go test: internal/rulerunner/api_stability_leak_test.go:638 apiStabilityProgram
// apiStabilityProgram builds one virtual project and returns its program
// and checker without running any rule. The caller owns the returned cleanup,
// which releases the checker; sharing one checker across a sequence of queries
// is what the later-read regressions need.
//
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

// go test: internal/rulerunner/api_stability_leak_test.go:670 apiStabilityExport
// apiStabilityExport returns the named export of a source file.
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

/// Rust-only: Go's `text[d.Pos():d.End()]`.
fn diagnostic_text(d: P<Diagnostic>, text: &str) -> &str {
    &text[d.pos().unwrap_or_default() as usize..d.end().unwrap_or_default() as usize]
}

// go test: internal/rulerunner/api_stability_leak_test.go:682 TestApiStabilityLeakRelatedDeclaration
#[test]
fn test_api_stability_leak_related_declaration() {
    ets_go::testing::run(|t| {
        t.parallel();
        struct Case {
            name: &'static str,
            files: Vec<(&'static str, &'static str)>,
            file: &'static str,
            identifier: &'static str,
        }
        let cases = [
            Case {
                name: "local type",
                files: vec![(
                    "test.ts",
                    "
/** @stability experimental */
interface Experimental { value: string }
export interface Public { value: Experimental }
",
                )],
                file: "test.ts",
                identifier: "Experimental",
            },
            Case {
                name: "cross-file re-export",
                files: vec![
                    (
                        "dep.ts",
                        "
/** @stability unstable */
export interface Unstable { value: string }
export interface Public { value: Unstable }
",
                    ),
                    ("test.ts", r#"export { Public } from "./dep.js""#),
                ],
                file: "dep.ts",
                identifier: "Unstable",
            },
            Case {
                name: "tagged overload",
                files: vec![(
                    "test.ts",
                    "
interface Callable {
  /** @stability experimental */
  (value: string): string;
  (value: number): number;
}
export interface Public { call: Callable }
",
                )],
                file: "test.ts",
                identifier: "(value: string): string;",
            },
        ];
        for tc in &cases {
            t.run(tc.name, |t| {
                t.parallel();
                let (program, mut c) = api_stability_program(t, &tc.files, true);
                let sf = source_file(program, "/.src/test.ts");
                let mut leaks = Vec::new();
                for diagnostic in c.get_diagnostics_exported(&Context::background(), sf) {
                    if diagnostic.code() == 377_137 {
                        leaks.push(diagnostic);
                    }
                }
                if leaks.len() != 1 {
                    t.fatal(sprintf!("expected one leak, got %d", leaks.len()));
                }
                if leaks[0].file() != Some(sf)
                    || diagnostic_text(leaks[0], file_source(&tc.files, "test.ts")) != "Public"
                {
                    t.fatal(sprintln!("primary diagnostic must point to the Public export"));
                }
                let related = leaks[0].related_information();
                if related.len() != 1 {
                    t.fatal(sprintf!("expected one related declaration, got %d", related.len()));
                }
                if related[0].file() != program.get_source_file(&format!("/.src/{}", tc.file)) {
                    t.fatal(sprintln!("related diagnostic points to the wrong source file"));
                }
                let got = diagnostic_text(related[0], file_source(&tc.files, tc.file));
                if got != tc.identifier {
                    t.fatal(sprintf!("related location = %q, want %q", got, tc.identifier));
                }
                if !related[0].localize().contains("is declared") {
                    t.fatal(sprintf!("unexpected related message: %s", related[0].localize()));
                }
                drop(c);
            });
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:761 apiStabilityDiagnostics
// apiStabilityDiagnostics collects the checker diagnostics for one file
// plus the global diagnostics, excluding the rule's own reports so a control
// run can be compared exactly with an enabled run.
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

// go test: internal/rulerunner/api_stability_leak_test.go:785 apiStabilityProgramDiagnostics
// apiStabilityProgramDiagnostics collects the whole-program semantic and
// global diagnostics, excluding the rule's own reports.
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

// go test: internal/rulerunner/api_stability_leak_test.go:813 TestApiStabilityLeakWholeSurfaceSafety
// TestApiStabilityLeakWholeSurfaceSafety pins findings 1-4: a complete
// structured surface is never materialized when any declaration it would
// resolve (own members, inherited members, index values, alias defaults, wide
// tuples) can expand a recursive alias. The comparison against a disabled
// control includes global diagnostics, so the rule cannot add TS2589, and the
// reverse comparison ensures it cannot lose an ordinary diagnostic either.
#[test]
fn test_api_stability_leak_whole_surface_safety() {
    ets_go::testing::run(|t| {
        t.parallel();
        let p = |s: &str| format!("{API_STABILITY_RECURSIVE_PREFIX}{s}");

        t.run("recursive index value", |t| {
            t.parallel();
            let source =
                p("interface I<T> { [s: string]: Deep<T> }\nexport declare const x: I<string>\n");
            run_api_stability_safety_case(
                t,
                "recursive index value",
                &[("test.ts", &source)],
                &[],
                &[],
            );
        });

        t.run("inherited recursive index value", |t| {
            t.parallel();
            let source = p("interface Base<T> { [s: string]: Deep<T> }\ninterface I<T> extends Base<T> {}\nexport declare const x: I<string>\n");
            run_api_stability_safety_case(t, "inherited recursive index value", &[("test.ts", &source)], &[], &[]);
        });

        t.run("anonymous recursive index return", |t| {
            t.parallel();
            let source = p("declare function make<T>(): { [s: string]: Deep<T> }\nexport const x = make<string>()\n");
            run_api_stability_safety_case(t, "anonymous recursive index return", &[("test.ts", &source)], &[], &[]);
        });

        t.run("compound indexed callable return", |t| {
            t.parallel();
            let source = p("interface I<T> { v: Deep<T> }\ndeclare function make<T>(): () => T[\"v\" & keyof T]\nexport const x = make<I<string>>()\n");
            run_api_stability_safety_case(t, "compound indexed callable return", &[("test.ts", &source)], &[], &[]);
        });

        t.run("compound indexed parameter", |t| {
            t.parallel();
            let source = p("interface I<T> { v: Deep<T> }\ndeclare function make<T>(): (x: T[\"v\" & keyof T]) => void\nexport const x = make<I<string>>()\n");
            run_api_stability_safety_case(t, "compound indexed parameter", &[("test.ts", &source)], &[], &[]);
        });

        t.run("compound indexed nested return", |t| {
            t.parallel();
            let source = p("interface I<T> { v: Deep<T> }\ndeclare function make<T>(): () => { value: T[\"v\" & keyof T] }\nexport const x = make<I<string>>()\n");
            run_api_stability_safety_case(t, "compound indexed nested return", &[("test.ts", &source)], &[], &[]);
        });

        t.run("compound indexed alias return", |t| {
            t.parallel();
            let source = p("interface I<T> { v: Deep<T> }\ntype Read<T> = T[\"v\" & keyof T]\ndeclare function make<T>(): () => Read<T>\nexport const x = make<I<string>>()\n");
            run_api_stability_safety_case(t, "compound indexed alias return", &[("test.ts", &source)], &[], &[]);
        });

        t.run("wide tuple recursive member", |t| {
            t.parallel();
            let elements = "number,".repeat(4100);
            let source = p(&format!(
                "interface I<T> {{\n  v: [{elements}Deep<T>]\n}}\nexport declare const x: I<string>\n"
            ));
            run_api_stability_safety_case(t, "wide tuple recursive member", &[("test.ts", &source)], &[], &[]);
        });

        t.run("skipped declaration alias default", |t| {
            t.parallel();
            let dep = p("type D<T = Deep<string>> = T\nexport interface X { v: D }\n");
            run_api_stability_safety_case(
                t,
                "skipped declaration alias default",
                &[("test.ts", "export * as ns from \"./dep.js\"\n"), ("dep.d.ts", &dep)],
                &[],
                &[],
            );
        });

        t.run("generic default references earlier parameter", |t| {
            t.parallel();
            let source =
                p("interface I<A, B = Deep<A>> { v: B }\nexport declare const x: I<string>\n");
            run_api_stability_safety_case(
                t,
                "generic default references earlier parameter",
                &[("test.ts", &source)],
                &[],
                &[],
            );
        });
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:910 TestApiStabilityLeakCoverage
// TestApiStabilityLeakCoverage pins findings 5-8: bare generic binders
// and safe inferred closures expose their concrete dependency; flattened
// compound aliases keep their declared provenance; nested class statics stay
// shallow.
#[test]
fn test_api_stability_leak_coverage() {
    ets_go::testing::run(|t| {
        t.parallel();

        let prefix = "/** @stability experimental */\ninterface E { v: string }\n";
        let p = |s: &str| format!("{prefix}{s}");

        t.run("bare generic callable return leaky first", |t| {
            t.parallel();
            let source = p("declare function make<T>(): () => T\nexport const x = make<E>()\n");
            run_api_stability_safety_case(
                t,
                "bare generic callable return leaky first",
                &[("test.ts", &source)],
                &["`x` exposes `E`"],
                &[],
            );
        });

        t.run("bare generic callable return clean first", |t| {
            t.parallel();
            run_api_stability_safety_case(
                t,
                "bare generic callable return clean first",
                &[(
                    "test.ts",
                    "declare function make<T>(): () => T\nexport const x = make<E>()\n/** @stability experimental */\ninterface E { v: string }\n",
                )],
                &["`x` exposes `E`"],
                &[],
            );
        });

        t.run("inferred closure", |t| {
            t.parallel();
            let source = p("declare const e: E\nfunction make<T>(t: T) { return () => ({ v: t }) }\nexport const x = make(e)\n");
            run_api_stability_safety_case(t, "inferred closure", &[("test.ts", &source)], &["`x` exposes `E`"], &[]);
        });

        t.run("inferred direct closure", |t| {
            t.parallel();
            let source = p("declare const e: E\nfunction make<T>(t: T) { return () => t }\nexport const x = make(e)\n");
            run_api_stability_safety_case(t, "inferred direct closure", &[("test.ts", &source)], &["`x` exposes `E`"], &[]);
        });

        t.run("inferred nested closure", |t| {
            t.parallel();
            let source = p("declare const e: E\nfunction make<T>(t: T) { return () => () => t }\nexport const x = make(e)\n");
            run_api_stability_safety_case(t, "inferred nested closure", &[("test.ts", &source)], &["`x` exposes `E`"], &[]);
        });

        t.run("inferred property closure", |t| {
            t.parallel();
            let source = p("declare const e: E\nfunction make<T>(t: T) { return { f: () => t } }\nexport const x = make(e)\n");
            run_api_stability_safety_case(t, "inferred property closure", &[("test.ts", &source)], &["`x` exposes `E`"], &[]);
        });

        t.run("flattened union alias", |t| {
            t.parallel();
            run_api_stability_safety_case(
                t,
                "flattened union alias",
                &[("test.ts", "/** @stability experimental */\ntype A = string | number\nexport declare const x: A | boolean\n")],
                &["`x` exposes `A`"],
                &[],
            );
        });

        t.run("flattened intersection alias", |t| {
            t.parallel();
            run_api_stability_safety_case(
                t,
                "flattened intersection alias",
                &[("test.ts", "/** @stability experimental */\ntype A = { a: string } & { b: number }\nexport declare const x: A & { c: boolean }\n")],
                &["`x` exposes `A`"],
                &[],
            );
        });

        t.run("nested class static shallow", |t| {
            t.parallel();
            let source = p("class C { static value: E }\nexport interface X { ctor: typeof C }\n");
            run_api_stability_safety_case(
                t,
                "nested class static shallow",
                &[("test.ts", &source)],
                &[],
                &["`X` exposes `E`"],
            );
        });

        t.run("benign default references earlier parameter", |t| {
            t.parallel();
            let source = p("interface I<A, B = A> { v: B }\nexport declare const x: I<E>\n");
            run_api_stability_safety_case(
                t,
                "benign default references earlier parameter",
                &[("test.ts", &source)],
                &["`x` exposes `E`"],
                &[],
            );
        });

        t.run("declared infer conditional alias", |t| {
            t.parallel();
            let source =
                p("export type First<T> = T extends [infer U, ...unknown[]] ? E : never\n");
            run_api_stability_safety_case(
                t,
                "declared infer conditional alias",
                &[("test.ts", &source)],
                &["`First` exposes `E`"],
                &[],
            );
        });

        t.run("materialized infer conditional outcome", |t| {
            t.parallel();
            let source = p("export declare const x: string[] extends Array<infer U> ? E : never\n");
            run_api_stability_safety_case(
                t,
                "materialized infer conditional outcome",
                &[("test.ts", &source)],
                &["`x` exposes `E`"],
                &[],
            );
        });
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:1019 TestApiStabilityLeakBareBinderOrders
// TestApiStabilityLeakBareBinderOrders exercises the bare generic binder
// return with actual `make<string>()` / `make<E>()` applications across clean
// and leaky export orders on one shared checker. Five repeated queries per
// order pin that a shared cache never lets the concrete `string` application
// contaminate the `E` application or the other way around.
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

// go test: internal/rulerunner/api_stability_leak_test.go:1075 TestApiStabilityLeakSubsequentCheckerExactness
// TestApiStabilityLeakSubsequentCheckerExactness checks the checker with
// the rule enabled and disabled after the same read sequence: a namespace
// barrel checked before the file that uses the recursive dependency. Both the
// later diagnostics (the TS2589 the ordinary read produces) and the ordinary
// type of the use site must match the control exactly, so the pre-flight guard
// can neither poison a later type nor consume a later diagnostic.
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

// go test: internal/rulerunner/api_stability_leak_test.go:1159 TestApiStabilityLeakInferredForwardingDiagnostics
// TestApiStabilityLeakInferredForwardingDiagnostics checks that
// forwarding an ordinary inferred function resolves its body only through the
// checker's native lazy inference, which attributes diagnostics to the
// declaring file's own expressions: a later check of the declaring file must
// still report the function's own diagnostics, exactly as the rule-disabled
// control does, and the whole-program diagnostic set must be identical too.
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
                // The forwarding probe is only meaningful when the control actually
                // reports something at the later check.
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

// go test: internal/rulerunner/api_stability_leak_test.go:1203 TestApiStabilityLeakBlockedSurfaceNotCached
// TestApiStabilityLeakBlockedSurfaceNotCached checks that a surface whose
// recursive component the guard refuses is never reported as stable and that
// repeating the query does no checker work. It then resolves the ordinary
// checker read to prove the pre-flight guard left the checker untouched: the
// finite inherited index resolves to its exact `number` value (not the error
// `any` a poisoned checker would report) and the recursive index still reports
// its own ordinary TS2589, both matching a rule-disabled control.
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
                // The ordinary read must be exact and must not have been poisoned by
                // the analysis: the finite application resolves to its exact
                // declared value without a global diagnostic.
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

// go test: internal/rulerunner/api_stability_leak_test.go:1281 apiStabilityRecursiveAlias
// apiStabilityRecursiveAlias is the self-expanding alias the regression
// probes use. Any speculative resolution of an application of it trips the
// checker's instantiation depth guard, so a rule that approves it would add
// TS2589 to a program whose own checking does not produce one.
const API_STABILITY_RECURSIVE_ALIAS: &str = "type Deep<T> = T extends string ? Deep<T> : T\n";

// go test: internal/rulerunner/api_stability_leak_test.go:1290 TestApiStabilityLeakRelationSafety
// TestApiStabilityLeakRelationSafety pins that the conditional-branch
// relation is only asked after a relation-operand pre-flight has established
// that resolving the full projected operand surfaces is bounded. Every probe
// runs the rule-enabled and rule-disabled programs through the same checker
// sequence and compares the checked file diagnostics, the later use-site
// diagnostics and types, and the global diagnostics exactly, so the rule can
// neither add nor lose a compiler diagnostic nor change a later type.
#[test]
fn test_api_stability_leak_relation_safety() {
    ets_go::testing::run(|t| {
        t.parallel();
        let a = |s: &str| format!("{API_STABILITY_RECURSIVE_ALIAS}{s}");

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
                        // While the dependency is still unchecked, the refused
                        // recursive read reports an incomplete result; nothing
                        // computed is persisted for a later analysis.
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

// go test: internal/rulerunner/api_stability_leak_test.go:1422 TestApiStabilityLeakRefusedSurfaceStaysRetryable
// TestApiStabilityLeakRefusedSurfaceStaysRetryable repeats the direct
// symbol query on one shared checker: a refused recursive surface always
// reports an incomplete result, and every repeated query after the first
// performs no checker instantiation, which proves the guard failed closed
// instead of settling a partial result.
#[test]
fn test_api_stability_leak_refused_surface_stays_retryable() {
    ets_go::testing::run(|t| {
        t.parallel();
        let a = |s: &str| format!("{API_STABILITY_RECURSIVE_ALIAS}{s}");

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

// go test: internal/rulerunner/api_stability_leak_test.go:1468 TestApiStabilityLeakExplicitStableForwarding
// TestApiStabilityLeakExplicitStableForwarding pins that a forwarding
// `@stability stable` tag on a re-export is an explicit ceiling, distinct from
// an absent tag: it tightens the forwarded symbol's inherited stability even
// though stable is the default tier. Named, star and namespace re-exports are
// covered.
#[test]
fn test_api_stability_leak_explicit_stable_forwarding() {
    ets_go::testing::run(|t| {
        t.parallel();

        let dep = "/** @stability experimental */
interface E { v: string }
/** @stability experimental */
export interface X { v: E }";
        // Rust: (label, want, absentLeak, source). absentLeak is true when an untagged re-export still reports: a
        // namespace alias inherits the untagged module's stable default, while
        // named and star re-exports inherit the forwarded symbol's own tag.
        let forwards = [
            ("named", "`X` exposes `E`", false, r#"export { X } from "./dep.js""#),
            ("star", "`X` exposes `E`", false, r#"export * from "./dep.js""#),
            ("namespace", "`ns` exposes `E`", true, r#"export * as ns from "./dep.js""#),
        ];
        for (label, want, absent_leak, forward_source) in forwards {
            for tag in ["stable", "unstable", "experimental", ""] {
                let name = if tag.is_empty() {
                    format!("{label}/absent")
                } else {
                    format!("{label}/{tag}")
                };
                t.run(&name, |t| {
                    t.parallel();
                    let source = if tag.is_empty() {
                        forward_source.to_string()
                    } else {
                        format!("/** @stability {tag} */\n{forward_source}")
                    };
                    let files = [("dep.d.ts", dep), ("test.ts", source.as_str())];
                    let got = api_stability_lifecycle_diagnostics(t, &files, true, false);
                    let control = api_stability_lifecycle_diagnostics(t, &files, false, false);
                    // The rule may only add its own reports; every compiler
                    // diagnostic must be exactly the control's.
                    for message in &got {
                        if is_rule_message(message) {
                            continue;
                        }
                        if !api_stability_lifecycle_contains(&control, message) {
                            t.error(sprintf!(
                                "added compiler diagnostic %q\nrule=%v\ncontrol=%v",
                                message,
                                got,
                                control
                            ));
                        }
                    }
                    for message in &control {
                        if !api_stability_lifecycle_contains(&got, message) {
                            t.error(sprintf!(
                                "lost compiler diagnostic %q\nrule=%v\ncontrol=%v",
                                message,
                                got,
                                control
                            ));
                        }
                    }
                    match tag {
                        "stable" | "unstable" => {
                            if !api_stability_lifecycle_contains(&got, want) {
                                t.error(sprintf!("explicit %s ceiling ignored: %v", tag, got));
                            }
                        }
                        "experimental" => {
                            if api_stability_lifecycle_contains(&got, "exposes") {
                                t.error(sprintf!(
                                    "unexpected leak for experimental ceiling: %v",
                                    got
                                ));
                            }
                        }
                        // Rust: Go's `case "":`; the tag is one of the four above.
                        _ => {
                            if absent_leak != api_stability_lifecycle_contains(&got, "exposes") {
                                t.error(sprintf!(
                                    "absent ceiling leak=%v, want %v: %v",
                                    api_stability_lifecycle_contains(&got, "exposes"),
                                    absent_leak,
                                    got
                                ));
                            }
                        }
                    }
                });
            }
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:1544 TestApiStabilityLeakInferredReexportCoverage
// TestApiStabilityLeakInferredReexportCoverage pins that an ordinary
// inferred re-export is reported in both the per-file and the whole-program
// lifecycle, while the dependency's own diagnostics stay exactly the control's.
// Inference is the checker's native lazy path: its diagnostics are attributed
// to the declaring file's expressions, so a later check of that file observes
// the same diagnostics and types.
#[test]
fn test_api_stability_leak_inferred_reexport_coverage() {
    ets_go::testing::run(|t| {
        t.parallel();

        let prefix =
            "/** @stability experimental */\ninterface E { v: string }\ndeclare const e: E\n";
        let p = |s: &str| format!("{prefix}{s}");
        let deps = [
            ("function", p("export function f() { return e }")),
            (
                "generic closure",
                p("function make<T>(t: T) { return () => ({ v: t }) }\nexport const f = make(e)"),
            ),
            (
                "direct closure",
                p("function make<T>(t: T) { return () => t }\nexport const f = make(e)"),
            ),
        ];
        let barrels = [
            ("named", r#"export { f } from "./dep.js""#, "exposes `E`"),
            ("star", r#"export * from "./dep.js""#, "exposes `E`"),
            ("namespace", r#"export * as ns from "./dep.js""#, "exposes `E`"),
        ];
        for (barrel_name, barrel_source, barrel_want) in barrels {
            for (dep_name, dep_source) in &deps {
                t.run(&format!("{barrel_name}/{dep_name}"), |t| {
                    t.parallel();
                    let files = [("test.ts", barrel_source), ("dep.ts", dep_source.as_str())];
                    for whole_program in [false, true] {
                        let got =
                            api_stability_lifecycle_diagnostics(t, &files, true, whole_program);
                        let control =
                            api_stability_lifecycle_diagnostics(t, &files, false, whole_program);
                        if !api_stability_lifecycle_contains(&got, barrel_want) {
                            t.error(sprintf!(
                                "missing inferred leak whole=%v: %v",
                                whole_program,
                                got
                            ));
                        }
                        for message in &got {
                            if is_rule_message(message) {
                                continue;
                            }
                            if !api_stability_lifecycle_contains(&control, message) {
                                t.error(sprintf!(
                                    "added compiler diagnostic %q\nrule=%v\ncontrol=%v",
                                    message,
                                    got,
                                    control
                                ));
                            }
                        }
                        for message in &control {
                            if !api_stability_lifecycle_contains(&got, message) {
                                t.error(sprintf!(
                                    "lost compiler diagnostic %q\nrule=%v\ncontrol=%v",
                                    message,
                                    got,
                                    control
                                ));
                            }
                        }
                    }
                });
            }
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:1595 TestApiStabilityLeakInferredDiagnosticsStayExact
// TestApiStabilityLeakInferredDiagnosticsStayExact pins that reading an
// inferred re-export through native inference never changes the dependency's
// own diagnostics, including the TS2589 a genuinely recursive initializer
// produces. The per-file request and the whole-program run are both compared
// against the rule-disabled control exactly.
#[test]
fn test_api_stability_leak_inferred_diagnostics_stay_exact() {
    ets_go::testing::run(|t| {
        t.parallel();
        let a = |s: &str| format!("{API_STABILITY_RECURSIVE_ALIAS}{s}");

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

// go test: internal/rulerunner/api_stability_leak_test.go:1639 TestApiStabilityLeakRelationOperands
// TestApiStabilityLeakRelationOperands pins the relation
// preflight fixes. A named property reached through a nested callable parameter
// or return and an anonymous compound operand whose raw structure mentions a
// substituted parameter must be refused before the compiler relation is asked:
// enabling the rule adds no compiler diagnostic, loses none, and reports the
// refused reads as incomplete.
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
                        // The rule must report a refused surface as incomplete
                        // while the dependency is still unchecked. Once the
                        // checker itself materializes the inferred component below,
                        // completion is sound and is not asserted against.
                        let x = api_stability_export(t, &mut c, dep_file, "X");
                        let refused =
                            ets_typeparser::new_type_parser(program, &mut c).api_stability_used_by_symbol(x);
                        if !refused.incomplete() {
                            t.error("refused surface X was not reported incomplete before the dependency was checked");
                        }
                    }
                    // Check the dependency twice: a refused read must not gain or
                    // lose a diagnostic between the first and the repeated check.
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

// go test: internal/rulerunner/api_stability_leak_test.go:1788 TestApiStabilityLeakAccessors
// TestApiStabilityLeakAccessors pins that inferred accessors re-exported
// through named, star and namespace barrels are reported in the per-file and
// whole-program lifecycles, including class accessors and accessors inside
// generic closures. Accessor body diagnostics and recursive inference must stay
// exactly the control's.
#[test]
fn test_api_stability_leak_accessors() {
    ets_go::testing::run(|t| {
        t.parallel();

        const PREFIX: &str =
            "/** @stability experimental */\ninterface E { v: string }\ndeclare const e: E\n";
        let p = |s: &str| format!("{PREFIX}{s}");
        let r = |s: &str| format!("{API_STABILITY_RECURSIVE_PREFIX}{s}");
        // Rust: (name, dep, barrel, want, notWant).
        let cases: [(&str, String, &str, &[&str], &[&str]); 11] = [
            (
                "object accessor",
                p("export const f = { get v() { return e } }"),
                r#"export { f } from "./dep.js""#,
                &["`f` exposes `E`"],
                &[],
            ),
            (
                "star forwarded object accessor",
                p("export const f = { get v() { return e } }"),
                r#"export * from "./dep.js""#,
                &["`f` exposes `E`"],
                &[],
            ),
            (
                "namespace forwarded object accessor",
                p("export const f = { get v() { return e } }"),
                r#"export * as ns from "./dep.js""#,
                &["`ns` exposes `E`"],
                &[],
            ),
            (
                "class accessor",
                p("export class C { get value() { return e } }"),
                r#"export { C } from "./dep.js""#,
                &["`C` exposes `E`"],
                &[],
            ),
            (
                "generic closure accessor",
                p(
                    "function make<T>(t: T) { return { get v() { return t } } }\nexport const f = make(e)",
                ),
                r#"export { f } from "./dep.js""#,
                &["`f` exposes `E`"],
                &[],
            ),
            (
                "setter parameter accessor",
                p("export const f = { set v(x: E) {} }"),
                r#"export { f } from "./dep.js""#,
                &["`f` exposes `E`"],
                &[],
            ),
            (
                "accessor body name error",
                p("export const f = { get v() { return nope } }"),
                r#"export { f } from "./dep.js""#,
                &[],
                &["exposes"],
            ),
            (
                "annotated accessor with error body",
                p("export const f = { get v(): E { return nope } }"),
                r#"export { f } from "./dep.js""#,
                &["`f` exposes `E`"],
                &[],
            ),
            (
                "setter body error",
                p("export const f = { set v(x: number) { x.missing } }"),
                r#"export { f } from "./dep.js""#,
                &[],
                &["exposes"],
            ),
            (
                "recursive inferred accessor",
                r(
                    "declare function make<T>(): Deep<T>\nexport const f = { get v() { return make<string>() } }",
                ),
                r#"export { f } from "./dep.js""#,
                &[],
                &["exposes"],
            ),
            (
                "recursive class accessor",
                r(
                    "declare const deep: Deep<string>\nexport class C { get value() { return deep } }",
                ),
                r#"export { C } from "./dep.js""#,
                &[],
                &["exposes"],
            ),
        ];
        for (name, dep, barrel, want, not_want) in &cases {
            let files = [("test.ts", *barrel), ("dep.ts", dep.as_str())];
            run_api_stability_safety_case(t, name, &files, want, not_want);
        }
    });
}

// go test: internal/rulerunner/api_stability_leak_test.go:1874 apiStabilityAccessorDiagnostic
// apiStabilityAccessorDiagnostic serializes one diagnostic exactly so the enabled
// and disabled runs can be compared including positions, codes and messages.
fn api_stability_accessor_diagnostic(diagnostic: P<Diagnostic>) -> String {
    let mut name = "global".to_string();
    if let Some(file) = diagnostic.file() {
        name = file.file_name().to_string();
    }
    format!("{name}:{}", diagnostic.localize())
}

// go test: internal/rulerunner/api_stability_leak_test.go:1885 TestApiStabilityLeakAccessorDiagnosticsExact
// TestApiStabilityLeakAccessorDiagnosticsExact forces the accessor
// through the rule lifecycle and compares the exact diagnostics of the barrel,
// the dependency file (checked twice) and the globals with a disabled control.
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
