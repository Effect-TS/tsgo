// Port of internal/rulerunner/prefer_effect_array_test.go.

use std::sync::Arc;

use ets_etscore::{EffectPluginOptions, Severity};
use ets_go::testing::{on_checker_stack, run};
use ets_shim::P;
use ets_shim::compiler::ProgramOptions;
use ets_shim::core::context::Context;
use ets_shim::core::tspath::ComparePathsOptions;
use ets_shim::core::{CompilerOptions, ModuleKind, ModuleResolutionKind, ScriptTarget, Tristate};
use ets_shim::vfs::{FS, bundled, vfstest};
use rustc_hash::FxHashMap;

#[derive(Default)]
struct Case {
    name: &'static str,
    source: String,
    severity: Severity,
    want: Vec<String>,
}

// go test: internal/rulerunner/prefer_effect_array_test.go:20 TestPreferEffectArrayLifecycle
#[test]
fn test_prefer_effect_array_lifecycle() {
    on_checker_stack(|| {
        run(|t| {
            t.parallel();
            const SOURCE: &str = "const xs = [1]; xs.map(n => n);";
            const MESSAGE: &str = "Consider effect/Array.map instead of native Array.map. The callback type accepts (value, index), with no array parameter or thisArg. effect(preferEffectArray)";
            const IS_ARRAY_MESSAGE: &str = "Consider effect/Array.isArray instead of native Array.isArray. Narrows elements to unknown rather than any. effect(preferEffectArray)";
            let cases = [
                Case {
                    name: "default off",
                    source: SOURCE.to_string(),
                    ..Default::default()
                },
                Case {
                    name: "warning",
                    source: SOURCE.to_string(),
                    severity: Severity::Warning,
                    want: vec![["377140:0:19:22:", MESSAGE].concat()],
                },
                Case {
                    name: "error",
                    source: SOURCE.to_string(),
                    severity: Severity::Error,
                    want: vec![["377140:1:19:22:", MESSAGE].concat()],
                },
                Case {
                    name: "dot reference",
                    source: "const xs = [1]; const alias = xs.map; alias.call(xs, n => n);".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377140:0:33:36:", MESSAGE].concat()],
                },
                Case {
                    name: "bracket invocation",
                    source: "const xs = [1]; xs[\"map\"](n => n);".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377140:0:19:24:", MESSAGE].concat()],
                },
                Case {
                    name: "bracket reference",
                    source: "const xs = [1]; const alias = xs[\"map\"]; alias.call(xs, n => n);".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377140:0:33:38:", MESSAGE].concat()],
                },
                Case {
                    name: "constructor dot",
                    source: "Array.isArray([]);".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377140:0:6:13:", IS_ARRAY_MESSAGE].concat()],
                },
                Case {
                    name: "constructor bracket",
                    source: "Array[\"isArray\"]([]);".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377140:0:6:15:", IS_ARRAY_MESSAGE].concat()],
                },
                Case {
                    name: "optional bracket",
                    source: "declare const xs: number[] | undefined; xs?.[\"map\"](n => n);".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377140:0:45:50:", MESSAGE].concat()],
                },
                Case {
                    name: "generic bracket invocation",
                    source: "function constrained<T extends readonly number[]>(values: T) { values[\"map\"](n => n); }".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377140:0:70:75:", MESSAGE].concat()],
                },
                Case {
                    name: "generic bracket reference",
                    source: "function constrained<T extends readonly number[]>(values: T) { return values[\"map\"]; }".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377140:0:77:82:", MESSAGE].concat()],
                },
                Case {
                    name: "custom and mixed generic brackets",
                    source: r#"function custom<T extends { map: (f: (n: number) => number) => number[] }>(values: T) { values["map"]; }
function mixed<T extends readonly number[] | { map: (f: (n: number) => number) => number[] }>(values: T) { values["map"]; }
const xs = [1]; xs.map(n => n);"#.to_string(),
                    severity: Severity::Warning,
                    want: vec![["377140:0:248:251:", MESSAGE].concat()],
                },
                Case {
                    name: "escaped and template names",
                    source: "const xs = [1]; xs.m\\u0061p(n => n); xs[\"m\\u0061p\"](n => n); xs[`map`](n => n);".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377140:0:19:27:", MESSAGE].concat(), ["377140:0:40:50:", MESSAGE].concat(), ["377140:0:64:69:", MESSAGE].concat()],
                },
                Case {
                    name: "parenthesized delete target",
                    source: "declare const xs: Partial<Pick<number[], \"map\">>; delete (xs.map); xs.map;".to_string(),
                    severity: Severity::Warning,
                    want: vec![["377140:0:70:73:", MESSAGE].concat()],
                },
                Case {
                    name: "directive enables default off",
                    source: ["// @effect-diagnostics preferEffectArray:warning\n", SOURCE].concat(),
                    want: vec![["377140:0:68:71:", MESSAGE].concat()],
                    ..Default::default()
                },
                Case {
                    name: "directive disables warning",
                    source: ["// @effect-diagnostics preferEffectArray:off\n", SOURCE].concat(),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "next line disables one reference",
                    source: ["// @effect-diagnostics-next-line preferEffectArray:off\n", SOURCE, "\nxs.map(n => n);"].concat(),
                    severity: Severity::Warning,
                    want: vec![["377140:0:90:93:", MESSAGE].concat()],
                },
                Case {
                    name: "next line enables default off",
                    source: ["// @effect-diagnostics-next-line preferEffectArray:error\n", SOURCE, "\nxs.map(n => n);"].concat(),
                    want: vec![["377140:1:76:79:", MESSAGE].concat()],
                    ..Default::default()
                },
            ];
            for tc in &cases {
                t.run(tc.name, |t| {
                    t.parallel();
                    let mut severity = FxHashMap::default();
                    for r in ets_rules::ALL.iter() {
                        if r.name != "preferEffectArray" {
                            severity.insert(r.name.to_string(), Severity::Off);
                        }
                    }
                    if tc.severity != Severity::Off {
                        severity.insert("preferEffectArray".to_string(), tc.severity);
                    }
                    ets_etscheckerhooks::init();
                    let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(
                        [("/.src/test.ts", tc.source.as_str())],
                        true,
                    )));
                    let options = CompilerOptions {
                        effect: Some(Arc::new(EffectPluginOptions {
                            diagnostics: true,
                            diagnostic_severity: Some(severity),
                            ..Default::default()
                        }))
                        .into(),
                        target: ScriptTarget::ESNext,
                        module: ModuleKind::NodeNext,
                        module_resolution: ModuleResolutionKind::NodeNext,
                        strict: Tristate::True,
                        skip_default_lib_check: Tristate::True,
                        skip_lib_check: Tristate::True,
                        ..Default::default()
                    };
                    let config = ets_shim::tsoptions::new_parsed_command_line(
                        P::new(options),
                        vec!["/.src/test.ts".to_string()],
                        Vec::new(),
                        ComparePathsOptions {
                            use_case_sensitive_file_names: true,
                            current_directory: "/.src".to_string(),
                        },
                    );
                    let mut program_options = ProgramOptions::new(
                        P::new(config),
                        ets_shim::compiler::new_compiler_host(
                            "/.src",
                            fs,
                            &bundled::lib_path(),
                            None,
                            None,
                            None,
                        ),
                    );
                    program_options.single_threaded = Tristate::True;
                    let program = crate::new_test_program(t, program_options);
                    let mut got = Vec::new();
                    for diagnostic in program.get_semantic_diagnostics(&Context::background(), None)
                    {
                        got.push(format!(
                            "{}:{}:{}:{}:{}",
                            diagnostic.code(),
                            diagnostic.category() as i32,
                            diagnostic.pos().unwrap_or_default(),
                            diagnostic.end().unwrap_or_default(),
                            diagnostic.localize()
                        ));
                    }
                    if got != tc.want {
                        t.fatal(format!("diagnostics = {got:?}, want {:?}", tc.want));
                    }
                });
            }
        });
    });
}
