// Port of internal/rulerunner/map_map_error_to_map_both_test.go.

use std::sync::Arc;

use ets_bundledeffect::{EffectVersion, TestFS};
use ets_etscore::{EffectPluginOptions, Severity};
use ets_go::testing::{on_checker_stack, run};
use ets_shim::P;
use ets_shim::compiler::ProgramOptions;
use ets_shim::core::context::Context;
use ets_shim::core::tspath::ComparePathsOptions;
use ets_shim::core::{CompilerOptions, ModuleKind, ModuleResolutionKind, ScriptTarget, Tristate};
use ets_shim::vfs::vfstest::MapFile;
use ets_shim::vfs::{FS, bundled, vfstest};
use rustc_hash::FxHashMap;

/// Rust-only: one entry of the Go test's anonymous `cases` struct slice.
#[derive(Default)]
struct Case {
    name: &'static str,
    source: String,
    severity: Severity,
    use_rule: bool,
    disabled: bool,
    preset: &'static str,
    want: Vec<String>,
}

// go test: internal/rulerunner/map_map_error_to_map_both_test.go:22 TestMapMapErrorToMapBothLifecycle
#[test]
fn test_map_map_error_to_map_both_lifecycle() {
    on_checker_stack(|| {
        run(|t| {
            t.parallel();
            const PRELUDE: &str = "import { Effect } from \"effect\"\ndeclare const source: Effect.Effect<number, string>\n";
            const PAIR: &str = "export const value = source.pipe(Effect.map((n) => n + 1), Effect.mapError((e) => e.length))\n";
            const MESSAGE: &str = ":`Effect.map` + `Effect.mapError` transform the success and failure channels one step at a time; `Effect.mapBoth` expresses both transformations in a single step. effect(mapMapErrorToMapBoth)";
            // The diagnostic covers the second callee, Effect.mapError, of the pair.
            let at = |category: i32, offset: usize| -> String {
                let start =
                    offset + "export const value = source.pipe(Effect.map((n) => n + 1), ".len();
                format!(
                    "377153:{}:{}:{}{}",
                    category,
                    start,
                    start + "Effect.mapError".len(),
                    MESSAGE
                )
            };
            let base = PRELUDE.len();
            let source = format!("{PRELUDE}{PAIR}");
            let cases = [
                Case {
                    name: "default suggestion",
                    source: source.clone(),
                    use_rule: true,
                    want: vec![at(2, base)],
                    ..Default::default()
                },
                Case {
                    name: "recommended preset",
                    source: source.clone(),
                    preset: "recommended",
                    want: vec![at(2, base)],
                    ..Default::default()
                },
                Case {
                    name: "strict preset",
                    source: source.clone(),
                    preset: "strict",
                    want: vec![at(1, base)],
                    ..Default::default()
                },
                Case {
                    name: "style preset",
                    source: source.clone(),
                    preset: "style",
                    want: vec![at(0, base)],
                    ..Default::default()
                },
                Case {
                    name: "effect-native preset",
                    source: source.clone(),
                    preset: "effect-native",
                    ..Default::default()
                },
                Case {
                    name: "warning",
                    source: source.clone(),
                    severity: Severity::Warning,
                    want: vec![at(0, base)],
                    ..Default::default()
                },
                Case {
                    name: "error",
                    source: source.clone(),
                    severity: Severity::Error,
                    want: vec![at(1, base)],
                    ..Default::default()
                },
                Case { name: "off", source: source.clone(), ..Default::default() },
                Case {
                    name: "diagnostics disabled",
                    source,
                    severity: Severity::Warning,
                    disabled: true,
                    ..Default::default()
                },
                Case {
                    name: "directive disables warning",
                    source: format!(
                        "// @effect-diagnostics mapMapErrorToMapBoth:off\n{PRELUDE}{PAIR}"
                    ),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "next line disables one pair",
                    source: format!(
                        "{PRELUDE}// @effect-diagnostics-next-line mapMapErrorToMapBoth:off\n{PAIR}{}",
                        PAIR.replacen("value", "other", 1)
                    ),
                    severity: Severity::Warning,
                    want: vec![at(
                        0,
                        base + "// @effect-diagnostics-next-line mapMapErrorToMapBoth:off\n".len()
                            + PAIR.len(),
                    )],
                    ..Default::default()
                },
            ];
            for tc in &cases {
                t.run(tc.name, |t| {
                    t.parallel();
                    let mut severity = FxHashMap::default();
                    for r in ets_rules::ALL.iter() {
                        if r.name != "mapMapErrorToMapBoth" {
                            severity.insert(r.name.to_string(), Severity::Off);
                        }
                    }
                    if tc.use_rule {
                        severity.insert(
                            "mapMapErrorToMapBoth".to_string(),
                            ets_rules::MAP_MAP_ERROR_TO_MAP_BOTH.default_severity,
                        );
                    } else if !tc.preset.is_empty() {
                        for preset in ets_rules::metadata_presets() {
                            if preset.name == tc.preset {
                                // Rust: Go's map read gives the zero value, `SeverityOff`, for a missing rule.
                                severity.insert(
                                    "mapMapErrorToMapBoth".to_string(),
                                    preset
                                        .diagnostic_severity
                                        .get("mapMapErrorToMapBoth")
                                        .copied()
                                        .unwrap_or_default(),
                                );
                            }
                        }
                    } else {
                        severity.insert("mapMapErrorToMapBoth".to_string(), tc.severity);
                    }
                    let mut files = TestFS::default();
                    files.insert("/.src/test.ts".to_string(), MapFile::from(tc.source.clone()));
                    if let Err(err) = ets_bundledeffect::mount_effect(EffectVersion::V4, &mut files)
                    {
                        t.fatal(err.to_string());
                    }
                    ets_etscheckerhooks::init();
                    let fs: Arc<dyn FS> =
                        Arc::new(bundled::wrap_fs(vfstest::from_map(files, true)));
                    let options = CompilerOptions {
                        effect: Some(Arc::new(EffectPluginOptions {
                            diagnostics: !tc.disabled,
                            diagnostic_severity: Some(severity),
                            ..Default::default()
                        }))
                        .into(),
                        target: ScriptTarget::ES_NEXT,
                        module: ModuleKind::NODE_NEXT,
                        module_resolution: ModuleResolutionKind::NODE_NEXT,
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
                    let source_file = program.get_source_file("/.src/test.ts");
                    let mut got = Vec::new();
                    let mut diagnostics =
                        program.get_semantic_diagnostics(&Context::background(), source_file);
                    diagnostics.extend(
                        program.get_suggestion_diagnostics(&Context::background(), source_file),
                    );
                    for diagnostic in diagnostics {
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
