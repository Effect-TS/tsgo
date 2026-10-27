use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/node_cannot_be_effect_library_type.go:16 TypeParser.NodeCannotBeEffectLibraryType 4123650ef8c35c6b
    // NodeCannotBeEffectLibraryType reports whether node is an expression whose
    // type can never be an Effect library data type such as Effect, Stream or
    // Layer. Literals, functions, and operators that always produce a primitive
    // have no properties that could match. The `undefined` identifier is left out
    // because a local binding can shadow it. Object literals are only ruled out in
    // v4, where those types carry a "~effect/..." key; v3 detection is structural
    // and could match a plain object literal.
    pub fn node_cannot_be_effect_library_type(&mut self, node: P<Node>) -> bool {
        match node.kind() {
            Kind::ArrayLiteralExpression
            | Kind::ArrowFunction
            | Kind::FunctionExpression
            | Kind::NumericLiteral
            | Kind::BigIntLiteral
            | Kind::TemplateExpression
            | Kind::NoSubstitutionTemplateLiteral
            | Kind::TrueKeyword
            | Kind::FalseKeyword
            | Kind::NullKeyword
            | Kind::RegularExpressionLiteral
            | Kind::PrefixUnaryExpression
            | Kind::VoidExpression
            | Kind::TypeOfExpression => true,
            Kind::StringLiteral => {
                // A string literal property name stands for the property's type.
                node.parent().is_none_or(|parent| parent.name() != Some(node))
            }
            Kind::ObjectLiteralExpression => {
                if self.detect_effect_version() != EffectMajorVersion::V4 {
                    return false;
                }
                for &property in node.as_object_literal_expression().properties().nodes() {
                    let Some(name) = property.name() else {
                        return false;
                    };
                    if name.kind() == Kind::ComputedPropertyName
                        || name.text().starts_with("~effect/")
                    {
                        return false;
                    }
                }
                true
            }
            _ => false,
        }
    }
}
