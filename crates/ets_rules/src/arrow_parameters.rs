use ets_shim::P;
use ets_shim::ast::{self, Kind, ModifierFlags, Node};
use ets_typeparser::TypeParser;

// go: internal/rules/arrow_parameters.go:13 arrowIgnoresParameter b24fe43b71380210
// arrowIgnoresParameter reports whether handler is a synchronous, non-generic
// arrow that cannot observe the value it is called with. Restrict callbacks to
// arrows so the value cannot be observed through a function's arguments
// object. An unused plain parameter can be removed.
pub(crate) fn arrow_ignores_parameter(tp: &mut TypeParser<'_>, handler: Option<P<Node>>) -> bool {
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
