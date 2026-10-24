use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, Symbol, SymbolFlags};
use ets_shim::checker::{ObjectFlags, Type, TypeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::EffectMajorVersion;
use rustc_hash::FxHashMap;

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::This_object_literal_is_checked_as_an_instance_of_Schema_class_0_but_has_no_class_prototype_instanceof_checks_methods_and_Schema_encoding_can_fail_Use_0_make_or_new_0_instead_effect_schemaClassObjectLiteral;

// go: internal/rules/schema_class_object_literal.go:14 SchemaClassObjectLiteral
pub static SCHEMA_CLASS_OBJECT_LITERAL: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "schemaClassObjectLiteral",
    group: "correctness",
    description: "Warns when object literals are checked as Schema class instances without the class prototype",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut classes: FxHashMap<P<Symbol>, bool> = FxHashMap::default();

        // Rust: Go's recursive `isSchemaClassInstance` closure is a nested function taking what it captures.
        fn is_schema_class_instance(
            ctx: &mut Context<'_, '_>,
            classes: &mut FxHashMap<P<Symbol>, bool>,
            t: Option<P<Type>>,
        ) -> bool {
            let Some(t) = t else {
                return false;
            };
            if !t.object_flags().intersects(ObjectFlags::Class | ObjectFlags::Reference) {
                return false;
            }
            let Some(symbol) = t.symbol() else {
                return false;
            };
            if !symbol.flags().intersects(SymbolFlags::Class) {
                return false;
            }
            if let Some(&result) = classes.get(&symbol) {
                return result;
            }
            classes.insert(symbol, false);
            for &declaration in symbol.declarations() {
                if (declaration.kind() == Kind::ClassDeclaration
                    || declaration.kind() == Kind::ClassExpression)
                    && schema_class_instance_self_and_brand(ctx.type_parser, declaration)
                        .0
                        .is_some()
                {
                    classes.insert(symbol, true);
                    return true;
                }
            }
            let declared = ctx.checker().get_declared_type_of_symbol_exported(symbol);
            let base_types = ctx.checker().get_base_types_exported(declared);
            let result =
                base_types.iter().any(|&base| is_schema_class_instance(ctx, classes, Some(base)));
            classes.insert(symbol, result);
            classes[&symbol]
        }

        let mut diagnostics = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(
            ctx: &mut Context<'_, '_>,
            classes: &mut FxHashMap<P<Symbol>, bool>,
            node: P<Node>,
            diagnostics: &mut Vec<P<Diagnostic>>,
        ) -> bool {
            if node.kind() == Kind::ObjectLiteralExpression {
                let contextual_type = ctx.type_parser.cached_object_literal_contextual_type(node);
                let mut class_name = String::new();
                for member in
                    ctx.type_parser.unroll_union_members(contextual_type).unwrap_or_default()
                {
                    if member.flags().intersects(TypeFlags::Null | TypeFlags::Undefined) {
                        continue;
                    }
                    if !is_schema_class_instance(ctx, classes, Some(member)) {
                        class_name = String::new();
                        break;
                    }
                    if class_name.is_empty() {
                        class_name = member
                            .symbol()
                            .map(|symbol| symbol.name())
                            .unwrap_or_default()
                            .to_string();
                    }
                }
                if !class_name.is_empty()
                    && !schema_class_object_literal_in_v4_constructor(ctx, node)
                {
                    diagnostics.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ctx.get_error_range(node),
                        MESSAGE,
                        &[],
                        &[&class_name],
                    ));
                }
            }
            node.for_each_child(&mut |child| walk(ctx, classes, child, diagnostics));
            false
        }

        let root = ctx.source_file.as_node();
        walk(ctx, &mut classes, root, &mut diagnostics);
        diagnostics
    },
});

// go: internal/rules/schema_class_object_literal.go:83 schemaClassObjectLiteralInV4Constructor
pub(crate) fn schema_class_object_literal_in_v4_constructor(
    ctx: &mut Context<'_, '_>,
    node: P<Node>,
) -> bool {
    if ctx.type_parser.supported_effect_version() != EffectMajorVersion::V4 {
        return false;
    }
    // V4 Schema constructors build nested class instances from plain objects.
    let mut node = node;
    let mut parent = node.parent();
    while let Some(p) = parent {
        match p.kind() {
            Kind::ObjectLiteralExpression
            | Kind::PropertyAssignment
            | Kind::ArrayLiteralExpression
            | Kind::ParenthesizedExpression => {}
            Kind::CallExpression | Kind::NewExpression => {
                if !p.arguments().contains(&node) {
                    return false;
                }
                let Some(mut expression) = p.expression() else {
                    return false;
                };
                if p.kind() == Kind::CallExpression {
                    if expression.kind() != Kind::PropertyAccessExpression {
                        return false;
                    }
                    let access = expression.as_property_access_expression();
                    match access.name().text() {
                        "make" | "makeOption" | "makeEffect" => {}
                        _ => return false,
                    }
                    expression = access.expression();
                }
                let t = ctx.type_parser.get_type_at_location(expression);
                return ctx.type_parser.is_schema_type(t);
            }
            _ => return false,
        }
        (node, parent) = (p, p.parent());
    }
    false
}
