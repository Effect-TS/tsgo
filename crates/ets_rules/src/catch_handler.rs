use ets_shim::P;
use ets_shim::ast::{self, Kind, ModifierFlags, Node};
use ets_typeparser::{
    LazyExpressionFlags, PipingFlow, PipingFlowSubject, PipingFlowTransformation, TypeParser,
};

use crate::prefer_succeed_some_or_none::is_option_none_call;

// go: internal/rules/catch_handler.go:11 catchHandlerIgnoresError 378a622d08553fbc
// Restrict callbacks to arrows so the caught error cannot be observed through
// a function's arguments object. An unused plain parameter can be removed.
pub(crate) fn catch_handler_ignores_error(
    tp: &mut TypeParser<'_>,
    handler: Option<P<Node>>,
) -> bool {
    let Some(handler) = handler else {
        return false;
    };
    if handler.kind() != Kind::ArrowFunction
        || ast::get_combined_modifier_flags(handler).intersects(ModifierFlags::Async)
    {
        return false;
    }
    if let Some(params) = ets_typeparser::get_function_like_type_parameters(handler)
        && !params.nodes().is_empty()
    {
        return false;
    }
    let params = ets_typeparser::get_function_like_parameters(handler);
    let Some(params) = params.filter(|params| !params.nodes().is_empty()) else {
        return true;
    };
    if params.nodes().len() != 1 {
        return false;
    }
    let parameter = params.nodes()[0].as_parameter_declaration();
    if parameter.name().kind() != Kind::Identifier
        || parameter.initializer().is_some()
        || parameter.dot_dot_dot_token().is_some()
    {
        return false;
    }
    let Some(symbol) = tp.get_symbol_at_location(parameter.name()) else {
        return false;
    };

    // Rust: Go's recursive `usesParameter` closure is a nested function.
    fn uses_parameter(
        tp: &mut TypeParser<'_>,
        symbol: P<ets_shim::ast::Symbol>,
        node: P<Node>,
    ) -> bool {
        if node.kind() == Kind::ShorthandPropertyAssignment
            && tp.checker().get_shorthand_assignment_value_symbol(Some(node)) == Some(symbol)
        {
            return true;
        }
        if node.kind() == Kind::Identifier && tp.get_symbol_at_location(node) == Some(symbol) {
            return true;
        }
        node.for_each_child(&mut |child| uses_parameter(tp, symbol, child))
    }

    let arrow = handler.as_arrow_function();
    let body = handler.body().expect("an arrow function has a body");
    !uses_parameter(tp, symbol, body)
        && arrow.type_().is_none_or(|type_node| !uses_parameter(tp, symbol, type_node))
}

// go: internal/rules/catch_handler.go:49 catchHandlerReturnsSucceedNone e385e74113256572
// catchHandlerReturnsSucceedNone reports whether a catch handler's complete
// returned flow is Effect.succeedNone or Effect.succeed(Option.none()).
pub(crate) fn catch_handler_returns_succeed_none(
    tp: &mut TypeParser<'_>,
    handler: P<Node>,
) -> bool {
    let Some(lazy) = ets_typeparser::parse_lazy_expression(handler, LazyExpressionFlags::None)
    else {
        return false;
    };
    let flow = tp.longest_piping_flow_at(lazy.expression(), false);
    PipingFlow::matches_exactly(
        flow.as_ref(),
        tp,
        &|tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
            subject
                .node()
                .is_some_and(|node| tp.is_node_reference_to_effect_module_api(node, "succeedNone"))
        },
        &[],
    ) || PipingFlow::matches_exactly(
        flow.as_ref(),
        tp,
        &|tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
            is_option_none_call(tp, subject.node())
        },
        &[&|tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| {
            step.args().len() == 0
                && step
                    .type_arguments()
                    .is_none_or(|type_arguments| type_arguments.nodes().is_empty())
                && tp.is_node_reference_to_effect_module_api(step.callee(), "succeed")
        }],
    )
}
