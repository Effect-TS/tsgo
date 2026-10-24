use ets_shim::ast::{Kind, Node, NodeFlags, SymbolFlags};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/expression_stability.go:7 TypeParser.IsExpressionValueStableAtLocation
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
                let Some(declaration_node) = symbol.value_declaration() else {
                    return false;
                };
                if declaration_node.kind() != Kind::VariableDeclaration {
                    return false;
                }

                let declaration = declaration_node.as_variable_declaration();
                let Some(initializer) = declaration.initializer() else {
                    return false;
                };
                let Some(parent) = declaration_node.parent() else {
                    return false;
                };
                if parent.kind() != Kind::VariableDeclarationList {
                    return false;
                }
                if !parent.flags().intersects(NodeFlags::Const)
                    || ets_shim::ast::get_source_file_of_node(declaration_node)
                        != ets_shim::ast::get_source_file_of_node(location)
                {
                    return false;
                }

                // Symbol resolution already proves the declaration is visible here. A
                // const's value is stable across nested lexical scopes as long as its
                // initializer occurs before the use; the same-container restriction would
                // incorrectly reject module constants referenced inside a function.
                initializer.end() <= location.pos()
            }
            _ => false,
        }
    }
}
