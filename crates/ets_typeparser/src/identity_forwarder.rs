use ets_shim::ast::{Kind, Node, NodeList};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/identity_forwarder.go:17 TypeParser.UnwrapIdentityForwarder e1137554430df53e
    // UnwrapIdentityForwarder returns the callee of a one-parameter function
    // whose body is a piping flow that applies a single operator to that
    // parameter, any explicit type arguments on that operator, and the
    // forwarder's parameter declaration so that callers can apply stricter checks
    // (e.g. reject annotated parameters). All other expressions are returned
    // as-is with no type arguments and no parameter.
    //
    // For example, fn, value => fn(value), value => pipe(value, fn) and
    // value => value.pipe(fn) all normalize to fn.
    //
    // Rust: a transformation's callee is never nil, so Go's nil check on it drops.
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
        let parameter_symbol = self.get_symbol_at_location(parameter_declaration.name());
        if parameter_symbol.is_none() {
            return (Some(node), None, None);
        }

        let is_parameter = |tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| -> bool {
            let Some(argument) = subject.node().map(ets_shim::ast::skip_parentheses) else {
                return false;
            };
            if argument.kind() != Kind::Identifier {
                return false;
            }
            let argument_symbol = tp.get_symbol_at_location(argument);
            argument_symbol.is_some()
                && get_symbol_if_same_reference(tp.checker, parameter_symbol, argument_symbol)
                    .is_some()
        };
        let is_bare_operator = |_: &mut TypeParser<'_>,
                                transformation: &PipingFlowTransformation|
         -> bool { transformation.args().len() == 0 };
        let Some(flow) = self.longest_piping_flow_at(lazy.expression, false) else {
            return (Some(node), None, None);
        };
        if !PipingFlow::matches_exactly(Some(&flow), self, &is_parameter, &[&is_bare_operator]) {
            return (Some(node), None, None);
        }

        let forwarded = flow.transformations().at(0);
        (
            Some(ets_shim::ast::skip_parentheses(forwarded.callee())),
            forwarded.type_arguments(),
            Some(parameter_node),
        )
    }
}
