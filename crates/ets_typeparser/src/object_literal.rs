use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;

// go: internal/typeparser/object_literal.go:7 ObjectLiteralPropertyInitializer c026dc8e1f1fb5b1
// ObjectLiteralPropertyInitializer returns the initializer of an explicitly
// assigned identifier or string-literal property in an object literal.
pub fn object_literal_property_initializer(
    node: impl Into<Option<P<Node>>>,
    name: &str,
) -> Option<P<Node>> {
    let node = ets_shim::ast::skip_parentheses(node.into()?);
    if node.kind() != Kind::ObjectLiteralExpression {
        return None;
    }
    let object = node.as_object_literal_expression();
    for &property_node in object.properties().nodes() {
        if property_node.kind() != Kind::PropertyAssignment {
            continue;
        }
        let property = property_node.as_property_assignment();
        let property_name = property.name();
        match property_name.kind() {
            Kind::Identifier if property_name.as_identifier().text() == name => {
                return Some(property.initializer());
            }
            Kind::StringLiteral if property_name.as_string_literal().text() == name => {
                return Some(property.initializer());
            }
            _ => {}
        }
    }
    None
}
