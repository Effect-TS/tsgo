// Port of etscheckerhooks/unused_parameters_test.go.

use std::sync::Arc;

use ets_etscore::{EffectPluginOptions, Severity};
use ets_shim::P;
use ets_shim::compiler::{Program, ProgramOptions};
use ets_shim::core::context::Context;
use ets_shim::core::tspath::ComparePathsOptions;
use ets_shim::core::{CompilerOptions, ScriptTarget, Tristate};
use ets_shim::lsproto;
use ets_shim::vfs::{FS, bundled, vfstest};
use rustc_hash::FxHashMap;

// go test: etscheckerhooks/unused_parameters_test.go:20 TestEffectDiagnosticsPreserveUnusedPredicateParameterLSP
#[test]
fn test_effect_diagnostics_preserve_unused_predicate_parameter_lsp() {
    ets_go::testing::on_checker_stack(|| {
        ets_go::testing::run(|t| {
            t.parallel();
            ets_etscheckerhooks::init();
            for enabled in [false, true] {
                t.run(&format!("plugin enabled={enabled}"), |t| {
                    t.parallel();
                    let mut plugins = "[]";
                    if enabled {
                        plugins = r#"[{"name":"@effect/language-service"}]"#;
                    }
                    let content = format!(
                        r#"// @Filename: /tsconfig.json
{{"compilerOptions":{{"strict":true,"noUnusedLocals":true,"noUnusedParameters":true,"target":"ES2023","types":[],"plugins":{plugins}}},"files":["main.ts"]}}
// @Filename: /main.ts
export function unusedPredicate([|value|]: unknown): value is number {{
  return true;
}}"#
                    );
                    let mut f = ets_fourslash::new_fourslash(t, None, &content);
                    f.go_to_file(t, "/main.ts");
                    for _ in 0..3 {
                        f.verify_diagnostics(
                            t,
                            &[lsproto::Diagnostic {
                                code: Some(lsproto::IntegerOrString {
                                    integer: Some(6133),
                                    string: None,
                                }),
                                message: lsproto::StringOrMarkupContent {
                                    string: Some(
                                        "'value' is declared but its value is never read."
                                            .to_string(),
                                    ),
                                    markup_content: None,
                                },
                                range: f.ranges()[0].ls_range,
                                tags: Some(vec![lsproto::DiagnosticTag::Unnecessary]),
                                ..Default::default()
                            }],
                        );
                    }
                    f.done(t);
                });
            }
        });
    });
}

// go test: etscheckerhooks/unused_parameters_test.go:50 TestEffectDiagnosticsPreserveUnusedPredicateParameter
#[test]
fn test_effect_diagnostics_preserve_unused_predicate_parameter() {
    for enabled in [false, true] {
        let name = if enabled { "plugin enabled" } else { "plugin disabled" };
        let program = unused_parameter_program(
            enabled,
            true,
            "export function unusedPredicate(value: unknown): value is number {\n  return true;\n}",
        );
        let source_file = program.get_source_file("/src/main.ts");
        let ctx = Context::background();
        for request in 0..3 {
            let diagnostics = program.get_semantic_diagnostics(&ctx, source_file);
            assert_eq!(
                diagnostics.len(),
                1,
                "{name}: request {request}: expected TS6133, got {:?}",
                diagnostics.iter().map(|d| d.localize()).collect::<Vec<_>>()
            );
            let diagnostic = diagnostics[0];
            assert!(
                diagnostic.code() == 6133
                    && diagnostic.pos() == Some(32)
                    && diagnostic.end() == Some(37)
                    && diagnostic.localize() == "'value' is declared but its value is never read.",
                "{name}: request {request}: unexpected diagnostic: {}",
                diagnostic.localize()
            );
            let suggestions = program.get_suggestion_diagnostics(&ctx, source_file);
            assert!(
                suggestions.is_empty(),
                "{name}: request {request}: unexpected suggestions: {:?}",
                suggestions.iter().map(|d| d.localize()).collect::<Vec<_>>()
            );
        }
    }
}

// go test: etscheckerhooks/unused_parameters_test.go:80 TestEffectDiagnosticsRunWithoutUnusedChecking
#[test]
fn test_effect_diagnostics_run_without_unused_checking() {
    let program = unused_parameter_program(
        true,
        false,
        "export function unusedPredicate(value: unknown): value is number {\n  return true;\n}\nexport class LocalProblem extends Error {}",
    );
    let source_file = program.get_source_file("/src/main.ts");
    let ctx = Context::background();
    for request in 0..3 {
        let mut codes = Vec::new();
        for diagnostic in program.get_semantic_diagnostics(&ctx, source_file) {
            codes.push(diagnostic.code());
        }
        assert!(
            codes == [377_055],
            "request {request}: expected one extendsNativeError diagnostic, got {codes:?}"
        );
    }
}

// go test: etscheckerhooks/unused_parameters_test.go:98 unusedParameterProgram
// Rust: the test package imports etscheckerhooks, whose `init` registers the hooks: `ets_etscheckerhooks::init()`.
fn unused_parameter_program(
    plugin_enabled: bool,
    unused_enabled: bool,
    source: &str,
) -> &'static Program {
    ets_etscheckerhooks::init();
    let fs: Arc<dyn FS> =
        Arc::new(bundled::wrap_fs(vfstest::from_map([("/src/main.ts", source)], true)));
    let mut options = CompilerOptions {
        strict: Tristate::True,
        target: ScriptTarget::ES2023,
        no_unused_locals: ets_shim::core::bool_to_tristate(unused_enabled),
        no_unused_parameters: ets_shim::core::bool_to_tristate(unused_enabled),
        skip_default_lib_check: Tristate::True,
        ..Default::default()
    };
    if plugin_enabled {
        options.effect = Some(Arc::new(EffectPluginOptions {
            diagnostics: true,
            diagnostic_severity: Some(FxHashMap::from_iter([(
                "extendsNativeError".to_string(),
                Severity::Error,
            )])),
            ..Default::default()
        }))
        .into();
    }
    let config = ets_shim::tsoptions::new_parsed_command_line(
        P::new(options),
        vec!["/src/main.ts".to_string()],
        Vec::new(),
        ComparePathsOptions {
            use_case_sensitive_file_names: true,
            current_directory: "/".to_string(),
        },
    );
    let mut program_options = ProgramOptions::new(
        P::new(config),
        ets_shim::compiler::new_compiler_host("/", fs, &bundled::lib_path(), None, None, None),
    );
    program_options.single_threaded = Tristate::True;
    ets_shim::compiler::new_program(program_options)
}
