// Package typeparser provides Effect type detection and parsing utilities.
//
// Rust: `checker_shim` holds the Rust-only stand-ins for the shim accessors tsrs keeps private (the others are made
// public by _patches/tsrs/120-checker-typeparser-visibility.patch).

mod api_stability;
mod api_stability_checker_shim;
mod api_stability_inheritance;
#[cfg(test)]
mod api_stability_inheritance_test;
#[cfg(test)]
mod api_stability_reset_test;
mod api_stability_safety;
#[cfg(test)]
mod api_stability_safety_test;
#[cfg(test)]
mod api_stability_surface_test;
#[cfg(test)]
mod api_stability_test;
mod cause_type;
#[cfg(test)]
mod cause_type_test;
mod checked_expression_types;
#[cfg(test)]
mod checked_expression_types_test;
mod checker_shim;
mod constant_evaluation;
mod context_tag;
mod context_type;
#[cfg(test)]
mod context_type_test;
mod could_be_strict_effect;
#[cfg(test)]
mod could_be_strict_effect_test;
mod data_first_signature;
#[cfg(test)]
mod data_first_signature_test;
mod data_type;
mod discover;
mod duration_api;
mod effect_context;
#[cfg(test)]
mod effect_context_test;
mod effect_fn;
mod effect_fn_opportunity;
#[cfg(test)]
mod effect_fn_test;
mod effect_gen;
#[cfg(test)]
mod effect_gen_test;
mod effect_model_type;
mod effect_type;
#[cfg(test)]
mod effect_type_test;
mod effect_yieldable_type;
mod expected_and_real_type;
#[cfg(test)]
mod expected_and_real_type_test;
mod expression_stability;
#[cfg(test)]
mod expression_stability_test;
mod extends_context_service;
mod extends_context_tag;
mod extends_data_error;
#[cfg(test)]
mod extends_data_error_test;
mod extends_data_tagged;
mod extends_effect_model_class;
mod extends_effect_service;
mod extends_effect_tag;
#[cfg(test)]
mod extends_schema_brand_test;
mod extends_schema_class;
mod extends_schema_opaque;
mod extends_schema_tagged;
mod extends_sql_model_class;
mod function_module;
#[cfg(test)]
mod function_module_test;
mod function_node;
mod get_contextual_type;
#[cfg(test)]
mod get_contextual_type_test;
#[cfg(test)]
mod get_symbol_at_location_test;
mod get_type_at_location;
#[cfg(test)]
mod get_type_at_location_test;
mod global_error;
mod helpers;
#[cfg(test)]
mod helpers_test;
mod identity_forwarder;
#[cfg(test)]
mod identity_forwarder_test;
mod layer_type;
mod lazy;
mod lazy_expression;
#[cfg(test)]
mod lazy_expression_test;
#[cfg(test)]
mod location_independent_type_parser_test;
mod metric_api;
#[cfg(test)]
mod metric_api_test;
mod module_export_reference;
#[cfg(test)]
mod module_export_reference_test;
mod module_identifier;
#[cfg(test)]
mod module_identifier_test;
mod native_array;
#[cfg(test)]
mod native_array_test;
mod native_object;
#[cfg(test)]
mod native_object_test;
mod native_string;
#[cfg(test)]
mod native_string_test;
mod node_cannot_be_effect_library_type;
#[cfg(test)]
mod node_cannot_be_effect_library_type_test;
mod object_literal;
mod object_literal_contextual_type;
mod option_type;
mod packagejson;
mod pipeable;
mod pipeable_signature_shape;
mod piping_flow;
#[cfg(test)]
mod piping_flow_input_type_test;
#[cfg(test)]
mod piping_flow_sequence_test;
#[cfg(test)]
mod piping_flow_shape_test;
mod platform_runtime_api;
mod predicate_type;
#[cfg(test)]
mod predicate_type_test;
mod promise_type;
#[cfg(test)]
mod readonly_results_test;
mod reconstruct;
mod result_dispatch;
mod returning_dispatch;
#[cfg(test)]
mod returning_dispatch_test;
mod schema_type;
mod scope_type;
mod service_type;
#[cfg(test)]
mod service_type_test;
mod sql_model_type;
mod stream_type;
#[cfg(test)]
mod tsrs_patches_test;
mod type_equivalent_node;
#[cfg(test)]
mod type_equivalent_node_test;
mod type_parser;
#[cfg(test)]
mod type_parser_test;
mod unique_types_map;
mod unroll_members;
mod vitest_api;
mod yieldable_error;

pub use api_stability::*;
pub(crate) use api_stability_checker_shim::*;
pub use api_stability_checker_shim::{
    DeclaredIndexInfo, get_conditional_type_branch_node, get_declared_index_infos_of_symbol,
    get_resolved_conditional_type_branch, get_resolved_declared_type_of_symbol_if_materialized,
    get_resolved_type_of_symbol_if_materialized,
};
pub(crate) use api_stability_safety::*;
pub use checked_expression_types::*;
pub(crate) use checker_shim::*;
// Rust-only: the shim accessors `internal/rules` also calls (`checker.Checker_getSymbolOfDeclaration`,
// `checker.Checker_getSymbolIfSameReference`, `checker.Checker_getTypeFromTypeNode`).
pub use checker_shim::{
    get_resolved_type_from_type_node, get_symbol_if_same_reference, get_symbol_of_declaration,
};
pub use context_type::*;
pub use data_first_signature::*;
pub use discover::*;
pub use effect_context::*;
pub use effect_fn::*;
pub use effect_fn_opportunity::*;
pub use effect_gen::*;
pub use effect_type::*;
pub use expected_and_real_type::*;
pub use extends_context_service::*;
pub use extends_context_tag::*;
pub use extends_data_error::*;
pub use extends_data_tagged::*;
pub use extends_effect_model_class::*;
pub use extends_effect_service::*;
pub use extends_effect_tag::*;
pub use extends_schema_class::*;
pub use extends_schema_opaque::*;
pub use extends_schema_tagged::*;
pub use extends_sql_model_class::*;
pub use function_node::*;

pub use layer_type::*;
pub use lazy::*;
pub use lazy_expression::*;
pub(crate) use module_export_reference::*;
pub use module_identifier::*;
pub(crate) use native_array::*;
pub(crate) use native_object::*;
pub(crate) use native_string::*;
pub use object_literal::*;
pub(crate) use packagejson::*;
pub(crate) use pipeable_signature_shape::*;
pub use piping_flow::*;
pub use reconstruct::*;
pub use returning_dispatch::*;
pub use schema_type::*;
pub use scope_type::*;
pub use service_type::*;
pub use stream_type::*;
pub use type_equivalent_node::*;
pub use type_parser::*;
pub use unique_types_map::*;
pub(crate) use yieldable_error::*;
