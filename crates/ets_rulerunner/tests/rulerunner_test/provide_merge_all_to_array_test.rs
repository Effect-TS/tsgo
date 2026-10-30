// Port of internal/rulerunner/provide_merge_all_to_array_test.go.

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

// go test: internal/rulerunner/provide_merge_all_to_array_test.go:22 TestProvideMergeAllToArrayLifecycle 321353194aebae3f
#[test]
fn test_provide_merge_all_to_array_lifecycle() {
    on_checker_stack(|| {
        run(|t| {
            t.parallel();
            const PRELUDE: &str = "import { Context, Effect, Layer } from \"effect\"\n\
                class A extends Context.Service<A, { readonly a: number }>()(\"A\") {}\n\
                class B extends Context.Service<B, { readonly b: number }>()(\"B\") {}\n\
                declare const ALayer: Layer.Layer<A>\n\
                declare const BLayer: Layer.Layer<B>\n\
                declare const program: Effect.Effect<void, never, A | B>\n";
            const PROVIDE: &str = "export const value = program.pipe(Effect.provide(Layer.mergeAll(ALayer, BLayer)))\n";
            const MESSAGE: &str = ":Effect.provide accepts an array of layers and merges them itself; pass the layers as an array instead of wrapping them in Layer.mergeAll. effect(provideMergeAllToArray)";
            // The diagnostic covers the Layer.mergeAll call written as the provide argument.
            let at = |category: i32, offset: usize| -> String {
                let start = offset + "export const value = program.pipe(Effect.provide(".len();
                format!(
                    "377161:{}:{}:{}{}",
                    category,
                    start,
                    start + "Layer.mergeAll(ALayer, BLayer)".len(),
                    MESSAGE
                )
            };
            let base = PRELUDE.len();
            let source = format!("{PRELUDE}{PROVIDE}");
            const NEXT_LINE: &str = "// @effect-diagnostics-next-line provideMergeAllToArray:off\n";
            let cases = [
                Case {
                    name: "default off",
                    source: source.clone(),
                    use_rule: true,
                    ..Default::default()
                },
                Case {
                    name: "recommended preset",
                    source: source.clone(),
                    preset: "recommended",
                    ..Default::default()
                },
                Case {
                    name: "strict preset",
                    source: source.clone(),
                    preset: "strict",
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
                    source: source.clone(),
                    severity: Severity::Warning,
                    disabled: true,
                    ..Default::default()
                },
                Case {
                    name: "directive disables warning",
                    source: format!("// @effect-diagnostics provideMergeAllToArray:off\n{source}"),
                    severity: Severity::Warning,
                    ..Default::default()
                },
                Case {
                    name: "next line disables one call",
                    source: format!(
                        "{PRELUDE}{NEXT_LINE}{PROVIDE}{}",
                        PROVIDE.replacen("value", "other", 1)
                    ),
                    severity: Severity::Warning,
                    want: vec![at(0, base + NEXT_LINE.len() + PROVIDE.len())],
                    ..Default::default()
                },
            ];
            for tc in &cases {
                t.run(tc.name, |t| {
                    t.parallel();
                    let mut severity = FxHashMap::default();
                    for r in ets_rules::ALL.iter() {
                        if r.name != "provideMergeAllToArray" {
                            severity.insert(r.name.to_string(), Severity::Off);
                        }
                    }
                    if tc.use_rule {
                        severity.insert(
                            "provideMergeAllToArray".to_string(),
                            ets_rules::PROVIDE_MERGE_ALL_TO_ARRAY.default_severity,
                        );
                    } else if !tc.preset.is_empty() {
                        for preset in ets_rules::metadata_presets() {
                            if preset.name == tc.preset {
                                // Rust: Go's map index gives the zero `SeverityOff` for a rule the preset omits.
                                severity.insert(
                                    "provideMergeAllToArray".to_string(),
                                    preset
                                        .diagnostic_severity
                                        .get("provideMergeAllToArray")
                                        .copied()
                                        .unwrap_or(Severity::Off),
                                );
                            }
                        }
                    } else {
                        severity.insert("provideMergeAllToArray".to_string(), tc.severity);
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
                    let mut got = Vec::new();
                    for diagnostic in program
                        .get_semantic_diagnostics(&Context::background(), None)
                        .into_iter()
                        .chain(program.get_suggestion_diagnostics(&Context::background(), None))
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
