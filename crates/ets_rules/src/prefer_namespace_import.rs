use std::sync::LazyLock;

use ets_etscore::{Severity, TopLevelNamedReexportsMode};
use ets_importstyle::Style;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile, Symbol, SymbolFlags};
use ets_shim::checker::Checker;
use ets_shim::diagnostics as tsdiag;

// go: internal/rules/prefer_namespace_import.go:12 PreferNamespaceImport
pub static PREFER_NAMESPACE_IMPORT: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "preferNamespaceImport",
        group: "style",
        description: "Reports named imports from packages configured in namespaceImportPackages that should be namespace imports",
        default_severity: Severity::Off,
        supported_effect: vec!["v3", "v4"],
        codes: vec![
            tsdiag::X_0_is_imported_by_name_from_1_but_namespaceImportPackages_expects_a_namespace_import_Use_2_instead_effect_preferNamespaceImport.code(),
            tsdiag::X_0_is_imported_by_name_from_1_but_namespaceImportPackages_expects_a_namespace_import_of_the_module_that_declares_0_effect_preferNamespaceImport.code(),
        ],
        run: run_prefer_namespace_import,
    }
});

// go: internal/rules/prefer_namespace_import.go:25 runPreferNamespaceImport
pub(crate) fn run_prefer_namespace_import(ctx: &mut Context<'_, '_>) -> Vec<P<Diagnostic>> {
    let Some(options) = ctx.options else {
        return Vec::new();
    };
    if options.namespace_import_packages.is_empty() {
        return Vec::new();
    }
    let policy = ets_importstyle::new_policy(ctx.options);
    let follow_reexports = options.top_level_named_reexports == TopLevelNamedReexportsMode::Follow;

    let mut diags = Vec::new();
    for &stmt in ctx.source_file.statements().nodes() {
        if stmt.kind() != Kind::ImportDeclaration {
            continue;
        }
        let import_decl = stmt.as_import_declaration();
        if import_decl.module_specifier.kind() != Kind::StringLiteral {
            continue;
        }
        let Some(clause) = import_decl.import_clause else {
            continue;
        };
        let Some(named_bindings) = clause.as_import_clause().named_bindings else {
            continue;
        };
        if named_bindings.kind() != Kind::NamedImports {
            continue;
        }
        let specifier = import_decl.module_specifier.text();
        if policy.style(specifier) != Style::Namespace {
            continue;
        }

        let mut module_symbol =
            ctx.type_parser.get_symbol_at_location(import_decl.module_specifier);
        if let Some(symbol) = module_symbol
            && symbol.flags().intersects(SymbolFlags::Alias)
        {
            module_symbol = Some(ctx.checker().get_aliased_symbol(symbol));
        }
        let Some(module_symbol) = module_symbol else {
            continue;
        };
        let module_file = symbol_source_file(module_symbol);

        let elements = named_bindings.as_named_imports().elements;
        for &element in elements.nodes() {
            let import_specifier = element.as_import_specifier();
            let mut imported_name = element.name().unwrap();
            if let Some(property_name) = element.property_name() {
                imported_name = property_name;
            }
            if imported_name.kind() != Kind::Identifier {
                continue;
            }
            let Some(export_symbol) =
                ctx.checker().try_get_member_in_module_exports(imported_name.text(), module_symbol)
            else {
                continue;
            };

            let (target, is_reexport) = namespace_import_target(
                ctx.checker(),
                export_symbol,
                module_file,
                specifier,
                element.name().unwrap().text(),
            );
            if is_reexport && !follow_reexports {
                continue;
            }
            let type_only = clause.is_type_only() || import_specifier.is_type_only;
            let location = ctx.get_error_range(element);
            if target.specifier.is_empty() {
                diags.push(ctx.new_diagnostic(
                    ctx.source_file,
                    location,
                    &tsdiag::X_0_is_imported_by_name_from_1_but_namespaceImportPackages_expects_a_namespace_import_of_the_module_that_declares_0_effect_preferNamespaceImport,
                    &[],
                    &[imported_name.text(), specifier],
                ));
                continue;
            }
            diags.push(ctx.new_diagnostic(
                ctx.source_file,
                location,
                &tsdiag::X_0_is_imported_by_name_from_1_but_namespaceImportPackages_expects_a_namespace_import_Use_2_instead_effect_preferNamespaceImport,
                &[],
                &[imported_name.text(), specifier, &namespace_import_text(&target, type_only)],
            ));
        }
    }
    diags
}

// go: internal/rules/prefer_namespace_import.go:114 namespaceImport
// namespaceImport is the namespace import an offending named import should become.
// An empty specifier means the declaring module could not be derived.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct NamespaceImport {
    pub(crate) name: String,
    pub(crate) specifier: String,
}

// go: internal/rules/prefer_namespace_import.go:122 namespaceImportTarget
// namespaceImportTarget classifies an export of the module at specifier.
// isReexport is true for exports that the module forwards from another module
// by name or through `export *`; those follow topLevelNamedReexports.
pub(crate) fn namespace_import_target(
    c: &mut Checker,
    export_symbol: P<Symbol>,
    module_file: Option<P<SourceFile>>,
    specifier: &str,
    local_name: &str,
) -> (NamespaceImport, bool) {
    let declarations = export_symbol.declarations();
    if !export_symbol.flags().intersects(SymbolFlags::Alias) {
        for &declaration in declarations {
            if ast::get_source_file_of_node(declaration) == module_file {
                return (
                    NamespaceImport {
                        name: ets_importstyle::namespace_name(specifier).to_string(),
                        specifier: specifier.to_string(),
                    },
                    false,
                );
            }
        }
        return (NamespaceImport::default(), true);
    }

    if declarations.is_empty() || ast::get_source_file_of_node(declarations[0]) != module_file {
        return (NamespaceImport::default(), true);
    }
    let declaration = declarations[0];
    match declaration.kind() {
        Kind::NamespaceExport => {
            let target = reexport_target(declaration.parent(), specifier);
            return (NamespaceImport { name: local_name.to_string(), specifier: target }, false);
        }
        Kind::ExportSpecifier => {
            let export_declaration = declaration.parent().unwrap().parent().unwrap();
            if export_declaration.as_export_declaration().module_specifier.is_some() {
                let target = reexport_target(Some(export_declaration), specifier);
                return (
                    NamespaceImport {
                        name: ets_importstyle::namespace_name(&target).to_string(),
                        specifier: target,
                    },
                    true,
                );
            }
        }
        _ => {}
    }
    // `export { local }` and `export default local` alias a declaration that
    // may live in the module itself.
    if let Some(aliased) = c.get_immediate_aliased_symbol(export_symbol)
        && !aliased.flags().intersects(SymbolFlags::Alias)
    {
        return namespace_import_target(c, aliased, module_file, specifier, local_name);
    }
    (NamespaceImport::default(), true)
}

// go: internal/rules/prefer_namespace_import.go:157 reexportTarget
// reexportTarget derives the public specifier of the module an export
// declaration in the barrel at specifier forwards from.
pub(crate) fn reexport_target(export_declaration: Option<P<Node>>, specifier: &str) -> String {
    let Some(export_declaration) = export_declaration else {
        return String::new();
    };
    if export_declaration.kind() != Kind::ExportDeclaration
        || !ets_importstyle::is_barrel_file(
            ast::get_source_file_of_node(export_declaration).unwrap().file_name(),
        )
    {
        return String::new();
    }
    let Some(module_specifier) = export_declaration.as_export_declaration().module_specifier else {
        return String::new();
    };
    if module_specifier.kind() != Kind::StringLiteral {
        return String::new();
    }
    ets_importstyle::reexport_specifier(specifier, module_specifier.text())
}

// go: internal/rules/prefer_namespace_import.go:168 symbolSourceFile
pub(crate) fn symbol_source_file(symbol: P<Symbol>) -> Option<P<SourceFile>> {
    for &declaration in symbol.declarations() {
        if declaration.kind() == Kind::SourceFile {
            return Some(declaration.as_source_file_p());
        }
    }
    if let Some(declaration) = symbol.value_declaration() {
        return ast::get_source_file_of_node(declaration);
    }
    None
}

// go: internal/rules/prefer_namespace_import.go:180 namespaceImportText
pub(crate) fn namespace_import_text(target: &NamespaceImport, type_only: bool) -> String {
    let mut keyword = "import ";
    if type_only {
        keyword = "import type ";
    }
    format!("{keyword}* as {} from \"{}\"", target.name, target.specifier)
}
