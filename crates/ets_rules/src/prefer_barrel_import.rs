use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_importstyle::{BarrelReader, Style};
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind};
use ets_shim::diagnostics as tsdiag;

// go: internal/rules/prefer_barrel_import.go:11 PreferBarrelImport
pub static PREFER_BARREL_IMPORT: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "preferBarrelImport",
        group: "style",
        description: "Reports imports of individual modules from packages configured in barrelImportPackages that should be named imports from the package barrel",
        default_severity: Severity::Off,
        supported_effect: vec!["v3", "v4"],
        codes: vec![tsdiag::X_0_is_imported_directly_but_barrelImportPackages_expects_named_imports_from_the_barrel_1_Use_2_instead_effect_preferBarrelImport.code()],
        run: run_prefer_barrel_import,
    }
});

// go: internal/rules/prefer_barrel_import.go:21 runPreferBarrelImport
//
// Rust: `TypeParser::barrel_reader` is never nil (the compiler program always reads files), and the reader sits
// behind a lock (see there); Go's `ctx.Program.GetResolvedModuleFromModuleSpecifier` is the program's
// module-specifier host method of the same name.
pub(crate) fn run_prefer_barrel_import(ctx: &mut Context<'_, '_>) -> Vec<P<Diagnostic>> {
    let Some(options) = ctx.options else {
        return Vec::new();
    };
    if options.barrel_import_packages.is_empty() {
        return Vec::new();
    }
    let barrels = ctx.type_parser.barrel_reader();
    let policy = ets_importstyle::new_policy(ctx.options);

    let mut diags = Vec::new();
    for &stmt in ctx.source_file.statements().nodes() {
        if stmt.kind() != Kind::ImportDeclaration {
            continue;
        }
        let import_decl = stmt.as_import_declaration();
        let Some(import_clause) = import_decl.import_clause else {
            continue;
        };
        if import_decl.module_specifier.kind() != Kind::StringLiteral {
            continue;
        }
        let Some(named_bindings) = import_clause.as_import_clause().named_bindings else {
            continue;
        };
        let specifier = import_decl.module_specifier.text();
        if policy.style(specifier) != Style::Barrel {
            continue;
        }
        let Some(resolved) = ctx
            .program
            .as_module_specifier_generation_host()
            .get_resolved_module_from_module_specifier(
                ctx.source_file,
                import_decl.module_specifier,
            )
        else {
            continue;
        };
        let Some(barrel) =
            BarrelReader::find(Some(&mut *barrels.lock()), specifier, resolved.resolved_file_name)
        else {
            continue;
        };
        let mut binding = barrel.export_name.clone();
        if named_bindings.kind() == Kind::NamespaceImport {
            let local_name = named_bindings.name().unwrap().text();
            if local_name != barrel.export_name {
                binding = format!("{} as {}", barrel.export_name, local_name);
            }
        }
        let mut keyword = "import ";
        if import_clause.is_type_only() {
            keyword = "import type ";
        }
        diags.push(ctx.new_diagnostic(
            ctx.source_file,
            ctx.get_error_range(import_decl.module_specifier),
            &tsdiag::X_0_is_imported_directly_but_barrelImportPackages_expects_named_imports_from_the_barrel_1_Use_2_instead_effect_preferBarrelImport,
            &[],
            &[
                specifier,
                &barrel.specifier,
                &format!("{keyword}{{ {binding} }} from \"{}\"", barrel.specifier),
            ],
        ));
    }
    diags
}
