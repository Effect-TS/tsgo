use ets_shim::ast::{Kind, Node, NodeFlags, SymbolFlags};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/expression_stability.go:10 TypeParser.IsExpressionValueStableAtLocation a8d28de85fcae8d7
    // IsExpressionValueStableAtLocation reports whether evaluating expression at
    // location produces the same value as evaluating it at its original site.
    pub fn is_expression_value_stable_at_location(
        &mut self,
        expression: impl Into<Option<P<Node>>>,
        location: impl Into<Option<P<Node>>>,
    ) -> bool {
        let (Some(expression), Some(location)) = (expression.into(), location.into()) else {
            return false;
        };

        let expression = ets_shim::ast::skip_parentheses(expression);

        match expression.kind() {
            Kind::StringLiteral
            | Kind::NumericLiteral
            | Kind::BigIntLiteral
            | Kind::NoSubstitutionTemplateLiteral
            | Kind::TrueKeyword
            | Kind::FalseKeyword
            | Kind::NullKeyword => true,
            Kind::PrefixUnaryExpression => {
                // -1 and -1n are signed literals; no other unary forms are accepted.
                // Rust: Go's `unary.Operand != nil` always holds (the operand is not optional).
                let unary = expression.as_prefix_unary_expression();
                unary.operator() == Kind::MinusToken
                    && (unary.operand().kind() == Kind::NumericLiteral
                        || unary.operand().kind() == Kind::BigIntLiteral)
            }
            Kind::PropertyAccessExpression => {
                // Enum members cannot be reassigned; the receiver must be the enum itself.
                let access = expression.as_property_access_expression();
                if !ets_shim::ast::is_entity_name_expression(access.expression()) {
                    return false;
                }
                let enum_symbol = self.get_symbol_at_location(access.expression());
                let member = self.get_symbol_at_location(access.name());
                let (Some(enum_symbol), Some(member)) = (enum_symbol, member) else {
                    return false;
                };
                if !enum_symbol.flags().intersects(SymbolFlags::Enum)
                    || !member.flags().intersects(SymbolFlags::EnumMember)
                {
                    return false;
                }
                let Some(declaration) = member.value_declaration() else {
                    return false;
                };
                ets_shim::ast::is_enum_member(declaration)
                    && self.is_declared_before_location(declaration.parent(), location)
            }
            Kind::Identifier => {
                let symbol = self.get_symbol_at_location(expression);
                if expression.text() == "undefined" {
                    return symbol.is_some()
                        && symbol
                            == self.checker.get_global_symbol_exported(
                                "undefined",
                                SymbolFlags::Value,
                                None,
                            );
                }
                let Some(symbol) = symbol else {
                    return false;
                };
                if !self.checker.is_constant_variable(symbol) {
                    return false;
                }
                let Some(declaration) = symbol.value_declaration() else {
                    return false;
                };
                if !ets_shim::ast::is_variable_declaration(declaration) {
                    return false;
                }
                // A `declare const` has no initializer, but its value cannot be reassigned.
                if declaration.initializer().is_none()
                    && !declaration.flags().intersects(NodeFlags::Ambient)
                {
                    return false;
                }
                self.is_declared_before_location(Some(declaration), location)
            }
            _ => false,
        }
    }

    // go: internal/typeparser/expression_stability.go:75 TypeParser.isDeclaredBeforeLocation a77d7353b1800f30
    // isDeclaredBeforeLocation reports whether declaration's value already exists
    // when location is evaluated. The checker alone accepts later declarations used
    // from deferred function bodies, so the positional check is kept alongside it.
    pub(crate) fn is_declared_before_location(
        &mut self,
        declaration: Option<P<Node>>,
        location: P<Node>,
    ) -> bool {
        declaration.is_some_and(|declaration| {
            ets_shim::ast::get_source_file_of_node(declaration)
                == ets_shim::ast::get_source_file_of_node(location)
                && declaration.end() <= location.pos()
                && self.checker.is_block_scoped_name_declared_before_use(declaration, location)
        })
    }
}
