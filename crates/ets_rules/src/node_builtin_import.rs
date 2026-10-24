use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{EffectMajorVersion, TypeParser};
use rustc_hash::FxHashMap;

// go: internal/rules/node_builtin_import.go:14 moduleAlternative
#[derive(Clone, Copy)]
pub(crate) struct ModuleAlternative {
    pub(crate) alternative: &'static str,
    pub(crate) package: &'static str,
    pub(crate) module: &'static str,
}

// go: internal/rules/node_builtin_import.go:20 moduleAlternativesV3
pub(crate) static MODULE_ALTERNATIVES_V3: LazyLock<FxHashMap<&'static str, ModuleAlternative>> =
    LazyLock::new(|| {
        FxHashMap::from_iter([
            (
                "fs",
                ModuleAlternative {
                    alternative: "FileSystem",
                    package: "@effect/platform",
                    module: "fs",
                },
            ),
            (
                "node:fs",
                ModuleAlternative {
                    alternative: "FileSystem",
                    package: "@effect/platform",
                    module: "fs",
                },
            ),
            (
                "fs/promises",
                ModuleAlternative {
                    alternative: "FileSystem",
                    package: "@effect/platform",
                    module: "fs",
                },
            ),
            (
                "node:fs/promises",
                ModuleAlternative {
                    alternative: "FileSystem",
                    package: "@effect/platform",
                    module: "fs",
                },
            ),
            (
                "path",
                ModuleAlternative {
                    alternative: "Path",
                    package: "@effect/platform",
                    module: "path",
                },
            ),
            (
                "node:path",
                ModuleAlternative {
                    alternative: "Path",
                    package: "@effect/platform",
                    module: "path",
                },
            ),
            (
                "path/posix",
                ModuleAlternative {
                    alternative: "Path",
                    package: "@effect/platform",
                    module: "path",
                },
            ),
            (
                "node:path/posix",
                ModuleAlternative {
                    alternative: "Path",
                    package: "@effect/platform",
                    module: "path",
                },
            ),
            (
                "path/win32",
                ModuleAlternative {
                    alternative: "Path",
                    package: "@effect/platform",
                    module: "path",
                },
            ),
            (
                "node:path/win32",
                ModuleAlternative {
                    alternative: "Path",
                    package: "@effect/platform",
                    module: "path",
                },
            ),
            (
                "child_process",
                ModuleAlternative {
                    alternative: "CommandExecutor",
                    package: "@effect/platform",
                    module: "child_process",
                },
            ),
            (
                "node:child_process",
                ModuleAlternative {
                    alternative: "CommandExecutor",
                    package: "@effect/platform",
                    module: "child_process",
                },
            ),
            (
                "http",
                ModuleAlternative {
                    alternative: "HttpClient",
                    package: "@effect/platform",
                    module: "http",
                },
            ),
            (
                "node:http",
                ModuleAlternative {
                    alternative: "HttpClient",
                    package: "@effect/platform",
                    module: "http",
                },
            ),
            (
                "https",
                ModuleAlternative {
                    alternative: "HttpClient",
                    package: "@effect/platform",
                    module: "https",
                },
            ),
            (
                "node:https",
                ModuleAlternative {
                    alternative: "HttpClient",
                    package: "@effect/platform",
                    module: "https",
                },
            ),
            (
                "console",
                ModuleAlternative { alternative: "Console", package: "effect", module: "console" },
            ),
            (
                "node:console",
                ModuleAlternative { alternative: "Console", package: "effect", module: "console" },
            ),
            (
                "timers",
                ModuleAlternative { alternative: "Effect", package: "effect", module: "timers" },
            ),
            (
                "node:timers",
                ModuleAlternative { alternative: "Effect", package: "effect", module: "timers" },
            ),
            (
                "timers/promises",
                ModuleAlternative { alternative: "Effect", package: "effect", module: "timers" },
            ),
            (
                "node:timers/promises",
                ModuleAlternative { alternative: "Effect", package: "effect", module: "timers" },
            ),
            (
                "stream",
                ModuleAlternative { alternative: "Stream", package: "effect", module: "stream" },
            ),
            (
                "node:stream",
                ModuleAlternative { alternative: "Stream", package: "effect", module: "stream" },
            ),
            (
                "stream/promises",
                ModuleAlternative { alternative: "Stream", package: "effect", module: "stream" },
            ),
            (
                "node:stream/promises",
                ModuleAlternative { alternative: "Stream", package: "effect", module: "stream" },
            ),
            (
                "stream/web",
                ModuleAlternative { alternative: "Stream", package: "effect", module: "stream" },
            ),
            (
                "node:stream/web",
                ModuleAlternative { alternative: "Stream", package: "effect", module: "stream" },
            ),
        ])
    });

// go: internal/rules/node_builtin_import.go:51 moduleAlternativesV4
pub(crate) static MODULE_ALTERNATIVES_V4: LazyLock<FxHashMap<&'static str, ModuleAlternative>> =
    LazyLock::new(|| {
        FxHashMap::from_iter([
            (
                "fs",
                ModuleAlternative { alternative: "FileSystem", package: "effect", module: "fs" },
            ),
            (
                "node:fs",
                ModuleAlternative { alternative: "FileSystem", package: "effect", module: "fs" },
            ),
            (
                "fs/promises",
                ModuleAlternative { alternative: "FileSystem", package: "effect", module: "fs" },
            ),
            (
                "node:fs/promises",
                ModuleAlternative { alternative: "FileSystem", package: "effect", module: "fs" },
            ),
            ("path", ModuleAlternative { alternative: "Path", package: "effect", module: "path" }),
            (
                "node:path",
                ModuleAlternative { alternative: "Path", package: "effect", module: "path" },
            ),
            (
                "path/posix",
                ModuleAlternative { alternative: "Path", package: "effect", module: "path" },
            ),
            (
                "node:path/posix",
                ModuleAlternative { alternative: "Path", package: "effect", module: "path" },
            ),
            (
                "path/win32",
                ModuleAlternative { alternative: "Path", package: "effect", module: "path" },
            ),
            (
                "node:path/win32",
                ModuleAlternative { alternative: "Path", package: "effect", module: "path" },
            ),
            (
                "child_process",
                ModuleAlternative {
                    alternative: "ChildProcess",
                    package: "effect/process",
                    module: "child_process",
                },
            ),
            (
                "node:child_process",
                ModuleAlternative {
                    alternative: "ChildProcess",
                    package: "effect/process",
                    module: "child_process",
                },
            ),
            (
                "http",
                ModuleAlternative {
                    alternative: "HttpClient",
                    package: "effect/http",
                    module: "http",
                },
            ),
            (
                "node:http",
                ModuleAlternative {
                    alternative: "HttpClient",
                    package: "effect/http",
                    module: "http",
                },
            ),
            (
                "https",
                ModuleAlternative {
                    alternative: "HttpClient",
                    package: "effect/http",
                    module: "https",
                },
            ),
            (
                "node:https",
                ModuleAlternative {
                    alternative: "HttpClient",
                    package: "effect/http",
                    module: "https",
                },
            ),
            (
                "console",
                ModuleAlternative { alternative: "Console", package: "effect", module: "console" },
            ),
            (
                "node:console",
                ModuleAlternative { alternative: "Console", package: "effect", module: "console" },
            ),
            (
                "timers",
                ModuleAlternative { alternative: "Effect", package: "effect", module: "timers" },
            ),
            (
                "node:timers",
                ModuleAlternative { alternative: "Effect", package: "effect", module: "timers" },
            ),
            (
                "timers/promises",
                ModuleAlternative { alternative: "Effect", package: "effect", module: "timers" },
            ),
            (
                "node:timers/promises",
                ModuleAlternative { alternative: "Effect", package: "effect", module: "timers" },
            ),
            (
                "stream",
                ModuleAlternative { alternative: "Stream", package: "effect", module: "stream" },
            ),
            (
                "node:stream",
                ModuleAlternative { alternative: "Stream", package: "effect", module: "stream" },
            ),
            (
                "stream/promises",
                ModuleAlternative { alternative: "Stream", package: "effect", module: "stream" },
            ),
            (
                "node:stream/promises",
                ModuleAlternative { alternative: "Stream", package: "effect", module: "stream" },
            ),
            (
                "stream/web",
                ModuleAlternative { alternative: "Stream", package: "effect", module: "stream" },
            ),
            (
                "node:stream/web",
                ModuleAlternative { alternative: "Stream", package: "effect", module: "stream" },
            ),
            (
                "crypto",
                ModuleAlternative { alternative: "Crypto", package: "effect", module: "crypto" },
            ),
            (
                "node:crypto",
                ModuleAlternative { alternative: "Crypto", package: "effect", module: "crypto" },
            ),
        ])
    });

// go: internal/rules/node_builtin_import.go:84 NodeBuiltinImport
pub static NODE_BUILTIN_IMPORT: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "nodeBuiltinImport",
        group: "effectNative",
        description: "Warns when importing Node.js built-in modules that have Effect-native counterparts",
        default_severity: Severity::Off,
        supported_effect: vec!["v3", "v4"],
        codes: vec![tsdiag::This_module_reference_uses_the_2_module_the_corresponding_Effect_API_is_0_from_1_effect_nodeBuiltinImport.code()],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            let matches = analyze_node_builtin_import(ctx.type_parser, ctx.source_file);
            let mut diags = Vec::with_capacity(matches.len());
            for m in &matches {
                diags.push(ctx.new_diagnostic(
                    m.source_file,
                    m.location,
                    &tsdiag::This_module_reference_uses_the_2_module_the_corresponding_Effect_API_is_0_from_1_effect_nodeBuiltinImport,
                    &[],
                    &[&m.alternative, &m.package, &m.module],
                ));
            }
            diags
        },
    }
});

// go: internal/rules/node_builtin_import.go:109 NodeBuiltinImportMatch
#[derive(Clone)]
pub struct NodeBuiltinImportMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub alternative: String,
    pub package: String,
    pub module: String,
}

// go: internal/rules/node_builtin_import.go:117 AnalyzeNodeBuiltinImport
//
// Rust: Go's (ignored) checker argument is dropped.
pub fn analyze_node_builtin_import(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<NodeBuiltinImportMatch> {
    let mut alternatives = &*MODULE_ALTERNATIVES_V4;
    if tp.supported_effect_version() == EffectMajorVersion::V3 {
        alternatives = &*MODULE_ALTERNATIVES_V3;
    }

    let mut matches = Vec::new();

    for &stmt in sf.statements().nodes() {
        match stmt.kind() {
            Kind::ImportDeclaration => {
                let import_decl = stmt.as_import_declaration();
                if import_decl.module_specifier.kind() != Kind::StringLiteral {
                    continue;
                }
                let specifier = import_decl.module_specifier.text();
                if let Some(alt) = alternatives.get(specifier) {
                    matches.push(NodeBuiltinImportMatch {
                        source_file: sf,
                        location: ets_shim::scanner::get_error_range_for_node(
                            sf,
                            import_decl.module_specifier,
                        ),
                        alternative: alt.alternative.to_string(),
                        package: alt.package.to_string(),
                        module: alt.module.to_string(),
                    });
                }
            }

            Kind::VariableStatement => {
                let var_stmt = stmt.as_variable_statement();
                let decl_list = var_stmt.declaration_list.as_variable_declaration_list();
                for &decl_node in decl_list.declarations.nodes() {
                    let Some(initializer) = decl_node.initializer() else {
                        continue;
                    };
                    if initializer.kind() != Kind::CallExpression {
                        continue;
                    }
                    let call_expr = initializer.as_call_expression();
                    if call_expr.expression.kind() != Kind::Identifier {
                        continue;
                    }
                    if ets_shim::scanner::get_text_of_node(call_expr.expression) != "require" {
                        continue;
                    }
                    if call_expr.arguments.nodes().len() != 1 {
                        continue;
                    }
                    let arg = call_expr.arguments.nodes()[0];
                    if arg.kind() != Kind::StringLiteral {
                        continue;
                    }
                    let specifier = arg.text();
                    if let Some(alt) = alternatives.get(specifier) {
                        matches.push(NodeBuiltinImportMatch {
                            source_file: sf,
                            location: ets_shim::scanner::get_error_range_for_node(sf, arg),
                            alternative: alt.alternative.to_string(),
                            package: alt.package.to_string(),
                            module: alt.module.to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    matches
}
