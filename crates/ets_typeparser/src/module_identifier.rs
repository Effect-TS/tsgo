use ets_shim::ast::{Kind, SourceFile};
use ets_shim::core::P;
use ets_shim::scanner::get_text_of_node;

/// Rust-only: Go's `s = s[1 : len(s)-1]` when `s` starts with a quote, over bytes as Go slices strings. The text
/// is only compared with ASCII module names, so an unterminated literal whose last byte splits a character (which
/// Go keeps as invalid UTF-8) compares unequal either way.
fn strip_specifier_quotes(text: String) -> String {
    let bytes = text.as_bytes();
    if bytes.len() >= 2 && (bytes[0] == b'"' || bytes[0] == b'\'') {
        return String::from_utf8_lossy(&bytes[1..bytes.len() - 1]).into_owned();
    }
    text
}

// go: internal/typeparser/module_identifier.go:11 FindEffectModuleIdentifier
// FindEffectModuleIdentifier finds the local name for the Effect module from imports.
// It checks both namespace imports (import * as X from "effect/Effect") and
// named imports (import { Effect } from "effect"), falling back to "Effect".
pub fn find_effect_module_identifier(sf: impl Into<Option<P<SourceFile>>>) -> String {
    let Some(sf) = sf.into() else {
        return "Effect".to_string();
    };
    for &stmt in sf.statements().nodes() {
        if stmt.kind() != Kind::ImportDeclaration {
            continue;
        }
        let import_decl = stmt.as_import_declaration();
        let Some(import_clause) = import_decl.import_clause() else {
            continue;
        };
        let module_name = strip_specifier_quotes(get_text_of_node(import_decl.module_specifier()));

        let clause = import_clause.as_import_clause();
        let Some(named_bindings) = clause.named_bindings() else {
            continue;
        };

        // Check namespace import: import * as X from "effect/Effect"
        if module_name == "effect/Effect" && named_bindings.kind() == Kind::NamespaceImport {
            let ns_import = named_bindings.as_namespace_import();
            return get_text_of_node(ns_import.name());
        }

        // Check named imports: import { Effect } from "effect"
        if module_name == "effect" && named_bindings.kind() == Kind::NamedImports {
            let named_imports = named_bindings.as_named_imports();
            for &elem in named_imports.elements().nodes() {
                let spec = elem.as_import_specifier();
                let imported_name = match spec.property_name() {
                    Some(property_name) => get_text_of_node(property_name),
                    None => get_text_of_node(spec.name()),
                };
                if imported_name == "Effect" {
                    return get_text_of_node(spec.name());
                }
            }
        }
    }
    "Effect".to_string()
}

// go: internal/typeparser/module_identifier.go:67 FindModuleIdentifier
// FindModuleIdentifier resolves the imported identifier name for the given export
// from the "effect" package. Falls back to the provided exportName if not found.
// It checks named imports: import { ExportName as Alias } from "effect".
pub fn find_module_identifier(sf: impl Into<Option<P<SourceFile>>>, export_name: &str) -> String {
    let Some(sf) = sf.into() else {
        return export_name.to_string();
    };
    for &stmt in sf.statements().nodes() {
        if stmt.kind() != Kind::ImportDeclaration {
            continue;
        }
        let import_decl = stmt.as_import_declaration();
        let module_name = strip_specifier_quotes(get_text_of_node(import_decl.module_specifier()));

        let Some(import_clause) = import_decl.import_clause() else {
            continue;
        };
        let clause = import_clause.as_import_clause();
        let Some(named_bindings) = clause.named_bindings() else {
            continue;
        };

        // Check namespace import: import * as X from "effect/<exportName>"
        if module_name == format!("effect/{export_name}")
            && named_bindings.kind() == Kind::NamespaceImport
        {
            let ns_import = named_bindings.as_namespace_import();
            return get_text_of_node(ns_import.name());
        }

        // Check named imports: import { ExportName as Alias } from "effect"
        if module_name == "effect" && named_bindings.kind() == Kind::NamedImports {
            let named_imports = named_bindings.as_named_imports();
            for &elem in named_imports.elements().nodes() {
                let spec = elem.as_import_specifier();
                let imported_name = match spec.property_name() {
                    Some(property_name) => get_text_of_node(property_name),
                    None => get_text_of_node(spec.name()),
                };
                if imported_name == export_name {
                    return get_text_of_node(spec.name());
                }
            }
        }
    }
    export_name.to_string()
}

// go: internal/typeparser/module_identifier.go:129 FindModuleIdentifierForPackage
// FindModuleIdentifierForPackage resolves the imported identifier name for the given
// module from an arbitrary package. It checks:
//   - Namespace imports: import * as X from "<packageName>/<moduleName>"
//   - Named imports: import { <moduleName> as X } from "<packageName>"
//
// Falls back to moduleName if not found.
pub fn find_module_identifier_for_package(
    sf: impl Into<Option<P<SourceFile>>>,
    package_name: &str,
    module_name: &str,
) -> String {
    let Some(sf) = sf.into() else {
        return module_name.to_string();
    };
    for &stmt in sf.statements().nodes() {
        if stmt.kind() != Kind::ImportDeclaration {
            continue;
        }
        let import_decl = stmt.as_import_declaration();
        let specifier = strip_specifier_quotes(get_text_of_node(import_decl.module_specifier()));

        let Some(import_clause) = import_decl.import_clause() else {
            continue;
        };
        let clause = import_clause.as_import_clause();
        let Some(named_bindings) = clause.named_bindings() else {
            continue;
        };

        // Check namespace import: import * as X from "<packageName>/<moduleName>"
        if specifier == format!("{package_name}/{module_name}")
            && named_bindings.kind() == Kind::NamespaceImport
        {
            let ns_import = named_bindings.as_namespace_import();
            return get_text_of_node(ns_import.name());
        }

        // Check named imports: import { <moduleName> as X } from "<packageName>"
        if specifier == package_name && named_bindings.kind() == Kind::NamedImports {
            let named_imports = named_bindings.as_named_imports();
            for &elem in named_imports.elements().nodes() {
                let spec = elem.as_import_specifier();
                let imported_name = match spec.property_name() {
                    Some(property_name) => get_text_of_node(property_name),
                    None => get_text_of_node(spec.name()),
                };
                if imported_name == module_name {
                    return get_text_of_node(spec.name());
                }
            }
        }
    }
    module_name.to_string()
}
