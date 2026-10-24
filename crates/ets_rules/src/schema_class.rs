use ets_shim::P;
use ets_shim::ast::Node;
use ets_typeparser::TypeParser;

// go: internal/rules/schema_class.go:8 schemaClassInstanceSelfAndBrand
//
// Rust: Go's `(self, brand)` pair of nillable nodes; `self` is nil exactly when no schema class shape matched.
pub(crate) fn schema_class_instance_self_and_brand(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> (Option<P<Node>>, Option<P<Node>>) {
    if let Some(result) = tp.extends_schema_class(node) {
        return (Some(result.self_type_node()), result.brand_type_node());
    }
    if let Some(result) = tp.extends_schema_tagged_class(node) {
        return (Some(result.self_type_node()), result.brand_type_node());
    }
    if let Some(result) = tp.extends_schema_error(node) {
        return (Some(result.self_type_node()), result.brand_type_node());
    }
    if let Some(result) = tp.extends_schema_tagged_error(node) {
        return (Some(result.self_type_node()), result.brand_type_node());
    }
    (None, None)
}
