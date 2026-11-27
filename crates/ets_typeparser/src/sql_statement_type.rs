use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::checker::{ObjectFlags, Type, TypeFlags};
use ets_shim::core::P;
use rustc_hash::FxHashMap;

use crate::*;

// go: internal/typeparser/sql_statement_type.go:8 effectSqlStatementModuleDescriptor 05a4d98c715c941f
pub(crate) static EFFECT_SQL_STATEMENT_MODULE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::SqlStatement,
        "effect",
        Some(&is_effect_sql_statement_source_file),
    );

// go: internal/typeparser/sql_statement_type.go:10 isEffectSqlStatementSourceFile 6c9a42a94373e679
pub(crate) fn is_effect_sql_statement_source_file(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    for name in ["statement", "isFragment", "makeCompiler", "defaultEscape"] {
        if c.try_get_member_in_module_exports_and_properties(name, module_sym).is_none() {
            return false;
        }
    }
    true
}

// go: internal/typeparser/sql_statement_type.go:28 sqlConstructorCheck 5f6fbc81434ffd18
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SqlConstructorCheck {
    // go: internal/typeparser/sql_statement_type.go:30 sqlConstructorCheckInProgress c34b9270e6e25acf
    InProgress,
    Matched,
    Unmatched,
}

// go: internal/typeparser/sql_statement_type.go:36 sqlConstructorChecks 93233e924dfdca0f
pub(crate) type SqlConstructorChecks = FxHashMap<P<Type>, SqlConstructorCheck>;

impl TypeParser<'_> {
    // go: internal/typeparser/sql_statement_type.go:38 TypeParser.IsEffectSqlStatementConstructorTaggedTemplate 23ce2cc4660d6f47
    // Rust: Go's nil checks on `tp`, `tp.checker`, `node` and `tagged.Tag` drop; the Rust types make them non-null.
    pub fn is_effect_sql_statement_constructor_tagged_template(&mut self, node: P<Node>) -> bool {
        if node.kind() != Kind::TaggedTemplateExpression {
            return false;
        }
        let tagged = node.as_tagged_template_expression();
        let t = self.get_type_at_location(tagged.tag());
        self.extends_effect_sql_statement_constructor(t, &mut SqlConstructorChecks::default())
    }

    // go: internal/typeparser/sql_statement_type.go:49 TypeParser.extendsEffectSqlStatementConstructor 451fb8f11c98677e
    pub(crate) fn extends_effect_sql_statement_constructor(
        &mut self,
        t: Option<P<Type>>,
        checks: &mut SqlConstructorChecks,
    ) -> bool {
        let Some(t) = t else {
            return false;
        };
        let mut t = self.checker.get_apparent_type_exported(t);
        if t.flags().intersects(TypeFlags::Union) {
            for &member in t.types() {
                if !self.extends_effect_sql_statement_constructor(Some(member), checks) {
                    return false;
                }
            }
            return !t.types().is_empty();
        }
        if t.flags().intersects(TypeFlags::Intersection) {
            for &member in t.types() {
                if self.extends_effect_sql_statement_constructor(Some(member), checks) {
                    return true;
                }
            }
            return false;
        }
        if !t.flags().intersects(TypeFlags::Object) {
            return false;
        }
        if t.object_flags().intersects(ObjectFlags::Reference)
            && let Some(target) = t.target()
        {
            t = target;
        }
        if !t.object_flags().intersects(ObjectFlags::ClassOrInterface) {
            return false;
        }
        if let Some(&check) = checks.get(&t) {
            return check == SqlConstructorCheck::Matched;
        }
        checks.insert(t, SqlConstructorCheck::InProgress);
        if self.interface_extends_effect_sql_statement_constructor(t, checks) {
            checks.insert(t, SqlConstructorCheck::Matched);
            return true;
        }
        checks.insert(t, SqlConstructorCheck::Unmatched);
        false
    }

    // go: internal/typeparser/sql_statement_type.go:91 TypeParser.interfaceExtendsEffectSqlStatementConstructor f4efbfd620a290eb
    pub(crate) fn interface_extends_effect_sql_statement_constructor(
        &mut self,
        t: P<Type>,
        checks: &mut SqlConstructorChecks,
    ) -> bool {
        if self.is_effect_sql_statement_constructor_interface(t) {
            return true;
        }
        for &base in self.checker.get_base_types_exported(t) {
            if self.extends_effect_sql_statement_constructor(Some(base), checks) {
                return true;
            }
        }
        false
    }

    // go: internal/typeparser/sql_statement_type.go:103 TypeParser.isEffectSqlStatementConstructorInterface c54544c7b4495435
    pub(crate) fn is_effect_sql_statement_constructor_interface(&mut self, t: P<Type>) -> bool {
        let Some(sym) = t.symbol() else {
            return false;
        };
        for &decl in sym.declarations() {
            if decl.kind() != Kind::InterfaceDeclaration {
                continue;
            }
            let Some(name) = decl.name() else {
                continue;
            };
            if self.is_node_reference_to_module_export(
                name,
                &EFFECT_SQL_STATEMENT_MODULE_DESCRIPTOR,
                "Constructor",
            ) {
                return true;
            }
        }
        false
    }
}
