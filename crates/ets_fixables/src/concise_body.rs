use ets_rewriter::Tracker;
use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;

use crate::leading_token_kind;

// go: internal/fixables/concise_body.go:12 replacementForExpression a6341c34e609c782
// replacementForExpression returns the node that replaces position with value.
// When position is the concise body of an arrow function and value begins with
// an object literal (`{ a }`, `{ a } as const`, `{ a } satisfies T`), the body
// would parse as a block, so value is parenthesized.
pub(crate) fn replacement_for_expression(
    tracker: &Tracker,
    position: P<Node>,
    value: P<Node>,
) -> P<Node> {
    if !is_concise_arrow_body(position)
        || leading_token_kind(value) != Kind::ObjectLiteralExpression
    {
        return value;
    }
    let clone = tracker.deep_clone_node(Some(value)).unwrap();
    tracker.new_parenthesized_expression(clone)
}

// go: internal/fixables/concise_body.go:21 isConciseArrowBody 2892de7f070dd51b
// isConciseArrowBody reports whether node is the expression body of an arrow
// function.
pub(crate) fn is_concise_arrow_body(node: P<Node>) -> bool {
    node.parent()
        .is_some_and(|parent| parent.kind() == Kind::ArrowFunction && parent.body() == Some(node))
}
