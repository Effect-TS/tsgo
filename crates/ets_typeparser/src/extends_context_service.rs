use ets_shim::ast::Node;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/extends_context_service.go:8 ServiceMapServiceResult
// ServiceMapServiceResult holds the parsed result of a class extending Context.Service.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ServiceMapServiceResult {
    pub(crate) class_name: P<Node>,     // The class name identifier
    pub(crate) self_type_node: P<Node>, // The Self type argument node (first type arg of the inner call)
    pub(crate) key_string_literal: Option<P<Node>>, // The key string literal from the outer call's first argument, or nil
}

impl ServiceMapServiceResult {
    // go: internal/typeparser/extends_context_service.go:14 ServiceMapServiceResult.ClassName
    pub fn class_name(&self) -> P<Node> {
        self.class_name
    }

    // go: internal/typeparser/extends_context_service.go:16 ServiceMapServiceResult.SelfTypeNode
    pub fn self_type_node(&self) -> P<Node> {
        self.self_type_node
    }

    // go: internal/typeparser/extends_context_service.go:18 ServiceMapServiceResult.KeyStringLiteral
    pub fn key_string_literal(&self) -> Option<P<Node>> {
        self.key_string_literal
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/extends_context_service.go:30 TypeParser.ExtendsContextService
    // ExtendsContextService checks if a class declaration extends Context.Service<Self, Shape>()(key).
    // It detects the double-call pattern:
    //
    //	class X extends Context.Service<X, Shape>()("key") {}
    //
    // where the ExpressionWithTypeArguments.expression is a CallExpression (outer call)
    // whose own .expression is also a CallExpression (inner call) with type arguments,
    // and the inner call resolves to Context.Service.
    //
    // Returns nil if the class does not extend Context.Service.
    pub fn extends_context_service(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<ServiceMapServiceResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_service_map_service,
            class_node,
            |tp| {
                // Must have a name
                let class_name = class_node.name()?;

                let heritage_elements =
                    ets_shim::ast::get_extends_heritage_clause_elements(class_node);
                if heritage_elements.is_empty() {
                    return None;
                }

                for &element in heritage_elements {
                    let ewta = element.as_expression_with_type_arguments();

                    // The expression should be a CallExpression (the outer call)
                    let outer_call_node = ewta.expression();
                    if !ets_shim::ast::is_call_expression(outer_call_node) {
                        continue;
                    }
                    let outer_call = outer_call_node.as_call_expression();

                    // The outer call's expression should also be a CallExpression (the inner call)
                    let inner_call_node = outer_call.expression();
                    if !ets_shim::ast::is_call_expression(inner_call_node) {
                        continue;
                    }
                    let inner_call = inner_call_node.as_call_expression();

                    // The inner call must have type arguments (Context.Service<Self, Shape>())
                    let Some(type_arguments) = inner_call.type_arguments() else {
                        continue;
                    };
                    if type_arguments.nodes().is_empty() {
                        continue;
                    }

                    // Check if the inner call's expression resolves to Context.Service
                    if !tp.is_node_reference_to_effect_context_module_api(
                        inner_call.expression(),
                        "Service",
                    ) {
                        continue;
                    }

                    // Extract key string literal from outer call's first argument
                    let mut key_string_literal = None;
                    if let Some(&arg) = outer_call.arguments().nodes().first()
                        && ets_shim::ast::is_string_literal(arg)
                    {
                        key_string_literal = Some(arg);
                    }

                    return Some(ServiceMapServiceResult {
                        class_name,
                        self_type_node: type_arguments.nodes()[0],
                        key_string_literal,
                    });
                }

                None
            },
        )
    }
}
