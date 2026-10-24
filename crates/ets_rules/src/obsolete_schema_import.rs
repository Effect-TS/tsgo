use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{EffectMajorVersion, TypeParser};

// go: internal/rules/obsolete_schema_import.go:16 ObsoleteSchemaImport
pub static OBSOLETE_SCHEMA_IMPORT: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "obsoleteSchemaImport",
        group: "correctness",
        description: "Warns when importing @effect/schema in projects targeting Effect v4, where Schema is included directly in the effect package",
        default_severity: Severity::Warning,
        supported_effect: vec!["v4"],
        codes: vec![tsdiag::This_module_reference_imports_0_which_is_obsolete_in_Effect_v4_In_Effect_v4_Schema_is_provided_directly_by_Schema_from_effect_or_effect_SlashSchema_effect_obsoleteSchemaImport.code()],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            let matches = analyze_obsolete_schema_import(ctx.type_parser, Some(ctx.source_file));
            let mut diags = Vec::with_capacity(matches.len());
            for m in &matches {
                diags.push(ctx.new_diagnostic(
                    m.source_file,
                    m.location,
                    &tsdiag::This_module_reference_imports_0_which_is_obsolete_in_Effect_v4_In_Effect_v4_Schema_is_provided_directly_by_Schema_from_effect_or_effect_SlashSchema_effect_obsoleteSchemaImport,
                    &[],
                    &[&m.imported_specifier],
                ));
            }
            diags
        },
    }
});

// go: internal/rules/obsolete_schema_import.go:39 ObsoleteSchemaImportMatch
#[derive(Clone)]
pub struct ObsoleteSchemaImportMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub imported_specifier: String,
}

// go: internal/rules/obsolete_schema_import.go:45 AnalyzeObsoleteSchemaImport
//
// Rust: Go's (ignored) checker argument is dropped; the parser is never nil.
pub fn analyze_obsolete_schema_import(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<ObsoleteSchemaImportMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();

    let is_obsolete_schema_specifier = |specifier: &str| -> bool {
        specifier == "@effect/schema" || specifier.starts_with("@effect/schema/")
    };

    for &stmt in sf.statements().nodes() {
        match stmt.kind() {
            Kind::ImportDeclaration => {
                let import_decl = stmt.as_import_declaration();
                if import_decl.module_specifier.kind() != Kind::StringLiteral {
                    continue;
                }
                let specifier = import_decl.module_specifier.text();
                if is_obsolete_schema_specifier(specifier) {
                    matches.push(ObsoleteSchemaImportMatch {
                        source_file: sf,
                        location: ets_shim::scanner::get_error_range_for_node(
                            sf,
                            import_decl.module_specifier,
                        ),
                        imported_specifier: specifier.to_string(),
                    });
                }
            }

            Kind::ExportDeclaration => {
                let export_decl = stmt.as_export_declaration();
                let Some(module_specifier) = export_decl.module_specifier else {
                    continue;
                };
                if module_specifier.kind() != Kind::StringLiteral {
                    continue;
                }
                let specifier = module_specifier.text();
                if is_obsolete_schema_specifier(specifier) {
                    matches.push(ObsoleteSchemaImportMatch {
                        source_file: sf,
                        location: ets_shim::scanner::get_error_range_for_node(sf, module_specifier),
                        imported_specifier: specifier.to_string(),
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
                    if is_obsolete_schema_specifier(specifier) {
                        matches.push(ObsoleteSchemaImportMatch {
                            source_file: sf,
                            location: ets_shim::scanner::get_error_range_for_node(sf, arg),
                            imported_specifier: specifier.to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    matches
}
