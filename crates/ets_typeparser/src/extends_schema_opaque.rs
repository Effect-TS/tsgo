use ets_shim::ast::Node;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/extends_schema_opaque.go:6 SchemaOpaqueResult 079ecd8d1015a989
// SchemaOpaqueResult holds the type arguments from a class extending Schema.Opaque.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SchemaOpaqueResult {
    pub(crate) self_type_node: P<Node>,
    pub(crate) brand_type_node: Option<P<Node>>,
}

impl SchemaOpaqueResult {
    // go: internal/typeparser/extends_schema_opaque.go:11 SchemaOpaqueResult.SelfTypeNode f642b1691a5faf9b
    pub fn self_type_node(self) -> P<Node> {
        self.self_type_node
    }

    // go: internal/typeparser/extends_schema_opaque.go:13 SchemaOpaqueResult.BrandTypeNode 6521ed215f896038
    pub fn brand_type_node(self) -> Option<P<Node>> {
        self.brand_type_node
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/extends_schema_opaque.go:18 TypeParser.ExtendsSchemaOpaque 55a7465fde93dd27
    // ExtendsSchemaOpaque checks for the exact Schema.Opaque double-call heritage shape:
    //
    //	class X extends Schema.Opaque<X>()(schema) {}
    pub fn extends_schema_opaque(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<SchemaOpaqueResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_schema_opaque,
            class_node,
            |tp| {
                for &element in ets_shim::ast::get_extends_heritage_clause_elements(class_node) {
                    let extends_expression = element.as_expression_with_type_arguments();
                    if !ets_shim::ast::is_call_expression(extends_expression.expression()) {
                        continue;
                    }
                    let outer_call = extends_expression.expression().as_call_expression();
                    if !ets_shim::ast::is_call_expression(outer_call.expression()) {
                        continue;
                    }
                    let inner_call = outer_call.expression().as_call_expression();
                    let Some(type_arguments) = inner_call.type_arguments() else {
                        continue;
                    };
                    let type_arguments = type_arguments.nodes();
                    if type_arguments.is_empty() || type_arguments.len() > 2 {
                        continue;
                    }
                    if tp.is_node_reference_to_effect_schema_module_api(
                        inner_call.expression(),
                        "Opaque",
                    ) {
                        let mut result = SchemaOpaqueResult {
                            self_type_node: type_arguments[0],
                            brand_type_node: None,
                        };
                        if type_arguments.len() == 2 {
                            result.brand_type_node = Some(type_arguments[1]);
                        }
                        return Some(result);
                    }
                }
                None
            },
        )
    }
}
