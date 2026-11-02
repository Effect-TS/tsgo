use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;

// go: internal/typeparser/object_literal.go:11 ObjectLiteralPropertyInitializer d73c0e0506af05a9
// ObjectLiteralPropertyInitializer returns the initializer that a property of
// an object literal (under any parentheses, type assertions or satisfies) is
// known to hold: the last property assignment with that name, provided nothing
// after it can override it. A later spread, a later shorthand, method or
// accessor with the same name, or a later computed name that is not a literal
// makes the value unknown, and the result nil.
//
// Rust: Go's nil node and nil property-list checks are the type system's.
pub fn object_literal_property_initializer(
    node: impl Into<Option<P<Node>>>,
    name: &str,
) -> Option<P<Node>> {
    let node = ets_shim::ast::skip_outer_expressions(
        node.into()?,
        ets_shim::ast::OuterExpressionKinds::All,
    );
    if node.kind() != Kind::ObjectLiteralExpression {
        return None;
    }
    let mut initializer = None;
    for &property in node.as_object_literal_expression().properties().nodes() {
        if property.kind() == Kind::SpreadAssignment {
            initializer = None;
            continue;
        }
        let Some(property_name) = property.name() else {
            continue;
        };
        match ets_shim::ast::try_get_text_of_property_name(property_name) {
            None => initializer = None,
            Some(text) if text != name => {}
            Some(_) if property.kind() == Kind::PropertyAssignment => {
                initializer = Some(property.as_property_assignment().initializer());
            }
            Some(_) => initializer = None,
        }
    }
    initializer
}
