use ets_etscore::{
    BRANDED_SCHEMA_CLASS_CLASS, BRANDED_SCHEMA_CLASS_ERROR, BRANDED_SCHEMA_CLASS_TAGGED_CLASS,
    BRANDED_SCHEMA_CLASS_TAGGED_ERROR,
};
use ets_shim::P;
use ets_shim::ast::Node;
use ets_typeparser::TypeParser;

// go: internal/rules/schema_class.go:11 schemaClassInstanceSelfAndBrand 3802e81c5e118443
// schemaClassInstanceSelfAndBrand returns the Self and brand type arguments of a Schema
// class heritage, along with its kind (one of the etscore.BrandedSchemaClass* names).
//
// Rust: Go's `(self, brand, kind)` triple of nillable nodes and a string; `self` is nil (and `kind` is "")
// exactly when no schema class shape matched.
pub(crate) fn schema_class_instance_self_and_brand(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> (Option<P<Node>>, Option<P<Node>>, &'static str) {
    if let Some(result) = tp.extends_schema_class(node) {
        return (
            Some(result.self_type_node()),
            result.brand_type_node(),
            BRANDED_SCHEMA_CLASS_CLASS,
        );
    }
    if let Some(result) = tp.extends_schema_tagged_class(node) {
        return (
            Some(result.self_type_node()),
            result.brand_type_node(),
            BRANDED_SCHEMA_CLASS_TAGGED_CLASS,
        );
    }
    if let Some(result) = tp.extends_schema_error(node) {
        return (
            Some(result.self_type_node()),
            result.brand_type_node(),
            BRANDED_SCHEMA_CLASS_ERROR,
        );
    }
    if let Some(result) = tp.extends_schema_tagged_error(node) {
        return (
            Some(result.self_type_node()),
            result.brand_type_node(),
            BRANDED_SCHEMA_CLASS_TAGGED_ERROR,
        );
    }
    (None, None, "")
}
