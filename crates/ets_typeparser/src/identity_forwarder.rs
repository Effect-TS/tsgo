use ets_shim::ast::{Kind, Node, NodeList};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/identity_forwarder.go:15 TypeParser.UnwrapIdentityForwarder
    // UnwrapIdentityForwarder returns the callee of a one-argument function that
    // forwards its argument unchanged, any explicit type arguments on the
    // forwarded call, and the forwarder's parameter declaration so that callers
    // can apply stricter checks (e.g. reject annotated parameters). All other
    // expressions are returned as-is with no type arguments and no parameter.
    //
    // For example, both fn and value => fn(value) normalize to fn.
    pub fn unwrap_identity_forwarder(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> (Option<P<Node>>, Option<P<NodeList>>, Option<P<Node>>) {
        let Some(node) = node.into() else {
            return (None, None, None);
        };

        let node = ets_shim::ast::skip_parentheses(node);
        let Some(lazy) = parse_lazy_expression(node, LazyExpressionFlags::None) else {
            return (Some(node), None, None);
        };
        let params = lazy.params.unwrap_or_default();
        if params.len() != 1 {
            return (Some(node), None, None);
        }

        let parameter_node = params[0];
        if parameter_node.kind() != Kind::Parameter {
            return (Some(node), None, None);
        }
        let parameter_declaration = parameter_node.as_parameter_declaration();
        if parameter_declaration.name().kind() != Kind::Identifier
            || parameter_declaration.dot_dot_dot_token().is_some()
            || parameter_declaration.initializer().is_some()
        {
            return (Some(node), None, None);
        }

        let expression = ets_shim::ast::skip_parentheses(lazy.expression);
        if expression.kind() != Kind::CallExpression {
            return (Some(node), None, None);
        }
        let call = expression.as_call_expression();
        let arguments = call.arguments().nodes();
        if arguments.len() != 1 {
            return (Some(node), None, None);
        }

        let argument = ets_shim::ast::skip_parentheses(arguments[0]);
        if argument.kind() != Kind::Identifier {
            return (Some(node), None, None);
        }

        let parameter_symbol = self.get_symbol_at_location(parameter_declaration.name());
        let argument_symbol = self.get_symbol_at_location(argument);
        if parameter_symbol.is_none()
            || argument_symbol.is_none()
            || get_symbol_if_same_reference(self.checker, parameter_symbol, argument_symbol)
                .is_none()
        {
            return (Some(node), None, None);
        }

        (
            Some(ets_shim::ast::skip_parentheses(call.expression())),
            call.type_arguments(),
            Some(parameter_node),
        )
    }
}
