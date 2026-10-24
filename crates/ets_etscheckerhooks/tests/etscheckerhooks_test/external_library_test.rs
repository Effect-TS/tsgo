// Port of etscheckerhooks/external_library_test.go.

use std::sync::Arc;

use ets_etscore::{EffectPluginOptions, Severity};
use ets_shim::P;
use ets_shim::ast::Diagnostic;
use ets_shim::compiler::ProgramOptions;
use ets_shim::core::context::Context;
use ets_shim::core::tspath::ComparePathsOptions;
use ets_shim::core::{
    CompilerOptions, ModuleKind, ModuleResolutionKind, NewLineKind, ScriptTarget, Tristate,
};
use ets_shim::vfs::{FS, bundled, vfstest};
use rustc_hash::FxHashMap;

// go test: etscheckerhooks/external_library_test.go:20 TestEffectDiagnosticsSkipExternalLibrarySourceFiles
#[test]
fn test_effect_diagnostics_skip_external_library_source_files() {
    ets_go::testing::on_checker_stack(|| {
        ets_go::testing::run(|t| {
            t.parallel();
            ets_etscheckerhooks::init();

            let testfs = [
                (
                    "/src/main.ts",
                    r#"import { ExternalProblem } from "problem"

export class LocalProblem extends Error {}
export const value = ExternalProblem
"#,
                ),
                (
                    "/node_modules/problem/package.json",
                    r#"{"name":"problem","version":"1.0.0","types":"index.ts"}"#,
                ),
                ("/node_modules/problem/index.ts", "export class ExternalProblem extends Error {}"),
            ];

            let fs: Arc<dyn FS> = Arc::new(bundled::wrap_fs(vfstest::from_map(testfs, true)));
            let options = CompilerOptions {
                new_line: NewLineKind::LF,
                no_error_truncation: Tristate::True,
                target: ScriptTarget::ESNext,
                module: ModuleKind::CommonJS,
                module_resolution: ModuleResolutionKind::Node10,
                effect: Some(Arc::new(EffectPluginOptions {
                    diagnostics: true,
                    diagnostic_severity: Some(FxHashMap::from_iter([(
                        "extendsNativeError".to_string(),
                        Severity::Error,
                    )])),
                    ..Default::default()
                }))
                .into(),
                ..Default::default()
            };
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
                ets_shim::compiler::new_compiler_host(
                    "/",
                    fs,
                    &bundled::lib_path(),
                    None,
                    None,
                    None,
                ),
            );
            program_options.single_threaded = Tristate::True;
            let program = ets_shim::compiler::new_program(program_options);

            let Some(dependency_source) = program.get_source_file("/node_modules/problem/index.ts")
            else {
                t.fatal("expected dependency source file to be resolved");
            };
            if !program.is_source_file_from_external_library(dependency_source) {
                t.fatal("expected dependency source file to be marked as external library");
            }

            let diagnostics = program.get_semantic_diagnostics(&Context::background(), None);
            let mut local_effect_diagnostics = 0;
            let mut external_effect_diagnostics = 0;
            for diag in diagnostics {
                if !is_effect_diagnostic(diag) {
                    continue;
                }
                let Some(file) = diag.file() else {
                    continue;
                };
                let file_name = file.file_name();
                if file_name.contains("/node_modules/") {
                    external_effect_diagnostics += 1;
                } else if file_name == "/src/main.ts" {
                    local_effect_diagnostics += 1;
                }
            }

            if external_effect_diagnostics != 0 {
                t.fatal(format!(
                    "expected no Effect diagnostics from external libraries, got {external_effect_diagnostics}"
                ));
            }
            if local_effect_diagnostics == 0 {
                t.fatal("expected Effect diagnostics for project source file");
            }
        });
    });
}

// go test: etscheckerhooks/external_library_test.go:91 isEffectDiagnostic
fn is_effect_diagnostic(diag: P<Diagnostic>) -> bool {
    let code = diag.code();
    (377_000..378_000).contains(&code)
}
