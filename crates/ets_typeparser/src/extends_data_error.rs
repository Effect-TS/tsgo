use ets_shim::ast::Node;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/extends_data_error.go:5 DataErrorResult f76e7cf12f19f6f2
// Rust: the class name is Go's `classNode.Name()`, nil for an anonymous class.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DataErrorResult {
    pub(crate) class_name: Option<P<Node>>,
    pub(crate) fields_type_node: Option<P<Node>>,
}

impl DataErrorResult {
    // go: internal/typeparser/extends_data_error.go:10 DataErrorResult.ClassName e9b4d1751a7ee72c
    pub fn class_name(self) -> Option<P<Node>> {
        self.class_name
    }

    // go: internal/typeparser/extends_data_error.go:12 DataErrorResult.FieldsTypeNode c736b45f1fe0aaed
    pub fn fields_type_node(self) -> Option<P<Node>> {
        self.fields_type_node
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/extends_data_error.go:14 TypeParser.ExtendsDataError b6468c7438769271
    pub fn extends_data_error(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<DataErrorResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_data_error,
            class_node,
            |tp| {
                for &element in ets_shim::ast::get_extends_heritage_clause_elements(class_node) {
                    let ewta = element.as_expression_with_type_arguments();
                    if !tp.is_node_reference_to_effect_data_module_api(ewta.expression(), "Error") {
                        continue;
                    }
                    // Heritage namespace lookup for entity-name expressions can ignore a shadowing value binding.
                    if ets_shim::ast::is_entity_name_expression(ewta.expression()) {
                        let root = ets_shim::ast::get_first_identifier(ewta.expression());
                        if ets_shim::ast::is_identifier(root) {
                            let resolved = tp.checker.get_resolved_symbol_exported(root);
                            let value_symbol = tp.reference_symbol_from(root, Some(resolved));
                            let reference_symbol = tp.reference_symbol_at_node(root);
                            if !same_symbol_reference(tp.checker, value_symbol, reference_symbol) {
                                continue;
                            }
                        }
                    }
                    let mut fields_type_node = None;
                    if let Some(type_arguments) = ewta.type_arguments()
                        && let Some(&first) = type_arguments.nodes().first()
                    {
                        fields_type_node = Some(first);
                    }
                    return Some(DataErrorResult {
                        class_name: class_node.name(),
                        fields_type_node,
                    });
                }
                None
            },
        )
    }
}
