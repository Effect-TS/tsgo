use ets_shim::ast::{self, Kind, Node, SourceFile};
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

// go: internal/typeparser/module_identifier.go:13 ModuleReference 9c3f0bd9795283d4
// ModuleReference is the path of names that reaches a module from a file's
// imports, for example ["Effect"] or ["X", "Effect"] for import * as X from "effect".
//
// Rust: a newtype over Go's `[]string`, which derefs to the names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModuleReference(pub Vec<String>);

impl std::ops::Deref for ModuleReference {
    type Target = [String];
    fn deref(&self) -> &[String] {
        &self.0
    }
}

/// Rust-only: Go formats a `ModuleReference` with `%s` through its `String` method.
impl std::fmt::Display for ModuleReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.string())
    }
}

impl ModuleReference {
    /// Rust-only: Go's `ModuleReference{names...}` literal.
    pub fn of(names: &[&str]) -> ModuleReference {
        ModuleReference(names.iter().map(|name| (*name).to_string()).collect())
    }

    // go: internal/typeparser/module_identifier.go:16 ModuleReference.String 2f41b43f51ca846c
    // String joins the path with ".", as it is written in source code.
    pub fn string(&self) -> String {
        self.0.join(".")
    }

    // go: internal/typeparser/module_identifier.go:21 ModuleReference.Append 098f5f5a6a02eae8
    // Append returns a new reference that extends r with names, leaving r untouched.
    #[must_use]
    pub fn append(&self, names: &[&str]) -> ModuleReference {
        let mut result = self.0.clone();
        result.extend(names.iter().map(|name| (*name).to_string()));
        ModuleReference(result)
    }

    // go: internal/typeparser/module_identifier.go:27 ModuleReference.MatchesNode fac258ada79c72af
    // MatchesNode reports whether node spells out the reference: an identifier, a
    // property access chain or a qualified name with the same names.
    pub fn matches_node(&self, node: P<Node>) -> bool {
        let r = &self.0;
        let mut node = node;
        for i in (1..r.len()).rev() {
            let name;
            match node.kind() {
                Kind::PropertyAccessExpression => {
                    let access = node.as_property_access_expression();
                    (node, name) = (access.expression(), access.name());
                }
                Kind::QualifiedName => {
                    let qualified = node.as_qualified_name();
                    (node, name) = (qualified.left(), qualified.right());
                }
                _ => return false,
            }
            if name.kind() != Kind::Identifier || name.text() != r[i] {
                return false;
            }
        }
        !r.is_empty() && node.kind() == Kind::Identifier && node.text() == r[0]
    }
}

// go: internal/typeparser/module_identifier.go:49 FindEffectModuleReference 95a7a2aa7385119d
// FindEffectModuleReference finds the reference to the Effect module from the
// file's imports. See FindModuleReference.
pub fn find_effect_module_reference(sf: impl Into<Option<P<SourceFile>>>) -> ModuleReference {
    find_module_reference(sf, "Effect")
}

// go: internal/typeparser/module_identifier.go:55 FindModuleReference f127cb8662d5f5b7
// FindModuleReference finds the reference to the given module of the "effect"
// package from the file's imports. See FindModuleReferenceForPackage.
pub fn find_module_reference(
    sf: impl Into<Option<P<SourceFile>>>,
    export_name: &str,
) -> ModuleReference {
    find_module_reference_for_package(sf, "effect", export_name)
}

// go: internal/typeparser/module_identifier.go:68 FindModuleReferenceForPackage 3b45f3c175dd9bff
// FindModuleReferenceForPackage finds the reference to the given module of a
// package from the file's imports. It recognises:
//   - Namespace imports: import * as X from "<packageName>/<moduleName>" → X
//   - Named imports: import { <moduleName> as X } from "<packageName>" → X
//   - Namespace imports of the barrel: import * as X from "<packageName>" → X.<moduleName>
//
// The first two forms are preferred over the barrel form. Type-only imports
// are skipped, as they cannot be referenced from values. Falls back to
// moduleName if not found; no import is added.
pub fn find_module_reference_for_package(
    sf: impl Into<Option<P<SourceFile>>>,
    package_name: &str,
    module_name: &str,
) -> ModuleReference {
    let Some(sf) = sf.into() else {
        return ModuleReference::of(&[module_name]);
    };
    let mut barrel: Option<ModuleReference> = None;
    for &stmt in sf.statements().nodes() {
        if stmt.kind() != Kind::ImportDeclaration {
            continue;
        }
        let import_decl = stmt.as_import_declaration();
        let Some(import_clause) = import_decl.import_clause() else {
            continue;
        };
        let specifier = strip_specifier_quotes(get_text_of_node(import_decl.module_specifier()));

        let clause = import_clause.as_import_clause();
        let Some(named_bindings) = clause.named_bindings() else {
            continue;
        };

        if named_bindings.kind() == Kind::NamespaceImport {
            let ns_import = named_bindings.as_namespace_import();
            if ast::is_part_of_type_only_import_or_export_declaration(named_bindings) {
                continue;
            }
            let name = get_text_of_node(ns_import.name());
            // import * as X from "<packageName>/<moduleName>"
            if specifier == format!("{package_name}/{module_name}") {
                return ModuleReference(vec![name]);
            }
            // import * as X from "<packageName>"
            if specifier == package_name && barrel.is_none() {
                barrel = Some(ModuleReference(vec![name, module_name.to_string()]));
            }
            continue;
        }

        // import { <moduleName> as X } from "<packageName>"
        if specifier == package_name && named_bindings.kind() == Kind::NamedImports {
            let named_imports = named_bindings.as_named_imports();
            for &elem in named_imports.elements().nodes() {
                if ast::is_part_of_type_only_import_or_export_declaration(elem) {
                    continue;
                }
                let spec = elem.as_import_specifier();
                let imported_name = match spec.property_name() {
                    Some(property_name) => get_text_of_node(property_name),
                    None => get_text_of_node(spec.name()),
                };
                if imported_name == module_name {
                    return ModuleReference(vec![get_text_of_node(spec.name())]);
                }
            }
        }
    }
    if let Some(barrel) = barrel {
        return barrel;
    }
    ModuleReference::of(&[module_name])
}
