// Port of internal/rulerunner/effect_succeed_with_undefined_test.go.

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

#[derive(Default)]
struct Case {
    name: &'static str,
    source: String,
    severity: Severity,
    preset: &'static str,
    want: Vec<String>,
}

// go test: internal/rulerunner/effect_succeed_with_undefined_test.go:22 TestEffectSucceedWithUndefinedLifecycle 99ba4c7c8cc3bfca
#[test]
fn test_effect_succeed_with_undefined_lifecycle() {
    on_checker_stack(|| {
        run(|t| {
            t.parallel();
            const RULE_NAME: &str = "effectSucceedWithUndefined";
            const MESSAGE: &str = "`Effect.undefined` represents the same outcome as `Effect.succeed(undefined)` or `Effect.succeed(void 0)`. effect(effectSucceedWithUndefined)";
            const BODY: &str = "import { Effect } from \"effect\"\nexport const program = Effect.succeed(undefined)\n";
            let twice = format!("{BODY}export const again = Effect.succeed(undefined)\n");
            // expect formats the diagnostic on the n-th Effect.succeed(undefined) call of source.
            let expect = |source: &str, n: usize, category: i32| -> String {
                let mut start: i64 = -1;
                for _ in 0..n {
                    let from = (start + 1) as usize;
                    start += 1 + source[from..]
                        .find("Effect.succeed(undefined)")
                        .map_or(-1, |i| i as i64);
                }
                format!(
                    "377162:{}:{}:{}:{}",
                    category,
                    start,
                    start + "Effect.succeed".len() as i64,
                    MESSAGE
                )
            };
            const CATEGORY_WARNING: i32 = 0;
            const CATEGORY_ERROR: i32 = 1;
            const CATEGORY_SUGGESTION: i32 = 2;
            let next_line = twice.replacen(
                "export const program",
                "// @effect-diagnostics-next-line effectSucceedWithUndefined:off\nexport const program",
                1,
            );
            let cases = [
                Case {
                    name: "default suggestion",
                    source: BODY.to_string(),
                    want: vec![expect(BODY, 1, CATEGORY_SUGGESTION)],
                    ..Default::default()
                },
                Case {
                    name: "recommended preset",
                    source: BODY.to_string(),
                    preset: "recommended",
                    want: vec![expect(BODY, 1, CATEGORY_SUGGESTION)],
                    ..Default::default()
                },
                Case {
                    name: "style preset",
                    source: BODY.to_string(),
                    preset: "style",
                    want: vec![expect(BODY, 1, CATEGORY_WARNING)],
                    ..Default::default()
                },
                Case {
                    name: "strict preset",
                    source: BODY.to_string(),
                    preset: "strict",
                    want: vec![expect(BODY, 1, CATEGORY_ERROR)],
                    ..Default::default()
                },
                Case {
                    name: "correctness preset",
                    source: BODY.to_string(),
                    preset: "correctness",
                    ..Default::default()
                },
                Case {
                    name: "warning",
                    source: BODY.to_string(),
                    severity: Severity::Warning,
                    want: vec![expect(BODY, 1, CATEGORY_WARNING)],
                    ..Default::default()
                },
                Case {
                    name: "error",
                    source: BODY.to_string(),
                    severity: Severity::Error,
                    want: vec![expect(BODY, 1, CATEGORY_ERROR)],
                    ..Default::default()
                },
                Case {
                    name: "directive disables",
                    source: format!(
                        "// @effect-diagnostics effectSucceedWithUndefined:off\n{BODY}"
                    ),
                    ..Default::default()
                },
                Case {
                    name: "next line disables one call",
                    want: vec![expect(&next_line, 2, CATEGORY_SUGGESTION)],
                    source: next_line.clone(),
                    ..Default::default()
                },
            ];
            for tc in &cases {
                t.run(tc.name, |t| {
                    t.parallel();
                    let mut severity = FxHashMap::default();
                    for r in ets_rules::ALL.iter() {
                        if r.name != RULE_NAME {
                            severity.insert(r.name.to_string(), Severity::Off);
                        }
                    }
                    if tc.severity != Severity::Off {
                        severity.insert(RULE_NAME.to_string(), tc.severity);
                    }
                    if !tc.preset.is_empty() {
                        let mut found = false;
                        for preset in ets_rules::metadata_presets() {
                            if preset.name == tc.preset {
                                found = true;
                                severity.insert(
                                    RULE_NAME.to_string(),
                                    preset
                                        .diagnostic_severity
                                        .get(RULE_NAME)
                                        .copied()
                                        .unwrap_or(Severity::Off),
                                );
                            }
                        }
                        if !found {
                            t.fatal(format!("preset {} not found", tc.preset));
                        }
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
                            diagnostics: true,
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
                    for diagnostic in
                        program.get_suggestion_diagnostics(&Context::background(), None)
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
