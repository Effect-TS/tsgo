use ets_shim::ast::Node;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/extends_effect_service.go:8 EffectServiceResult
// EffectServiceResult holds the parsed result of a class extending Effect.Service.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EffectServiceResult {
    pub(crate) class_name: P<Node>,      // The class name identifier
    pub(crate) self_type_node: P<Node>, // The Self type argument node (first type arg of the inner call)
    pub(crate) options: Option<P<Node>>, // The options expression (second argument of the outer call), or nil
    pub(crate) key_string_literal: Option<P<Node>>, // The key string literal from the outer call's first argument, or nil
}

impl EffectServiceResult {
    // go: internal/typeparser/extends_effect_service.go:15 EffectServiceResult.ClassName
    pub fn class_name(&self) -> P<Node> {
        self.class_name
    }

    // go: internal/typeparser/extends_effect_service.go:17 EffectServiceResult.SelfTypeNode
    pub fn self_type_node(&self) -> P<Node> {
        self.self_type_node
    }

    // go: internal/typeparser/extends_effect_service.go:19 EffectServiceResult.Options
    pub fn options(&self) -> Option<P<Node>> {
        self.options
    }

    // go: internal/typeparser/extends_effect_service.go:21 EffectServiceResult.KeyStringLiteral
    pub fn key_string_literal(&self) -> Option<P<Node>> {
        self.key_string_literal
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/extends_effect_service.go:33 TypeParser.ExtendsEffectV3Service
    // ExtendsEffectV3Service checks if a class declaration extends Effect.Service<Self>()(key, options).
    // It detects the double-call pattern:
    //
    //	class X extends Effect.Service<X>()("key", { ... }) {}
    //
    // where the ExpressionWithTypeArguments.expression is a CallExpression (outer call)
    // whose own .expression is also a CallExpression (inner call) with type arguments,
    // and the inner call resolves to Effect.Service.
    //
    // Returns nil if the class does not extend Effect.Service.
    pub fn extends_effect_v3_service(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<EffectServiceResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_effect_service,
            class_node,
            |tp| {
                if tp.supported_effect_version() == EffectMajorVersion::V4 {
                    return None;
                }

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

                    // The inner call must have type arguments (Effect.Service<Self>())
                    let Some(type_arguments) = inner_call.type_arguments() else {
                        continue;
                    };
                    if type_arguments.nodes().is_empty() {
                        continue;
                    }

                    // Check if the inner call's expression resolves to Effect.Service
                    if !tp
                        .is_node_reference_to_effect_module_api(inner_call.expression(), "Service")
                    {
                        continue;
                    }

                    // Extract the key string literal from outer call's first argument
                    let outer_arguments = outer_call.arguments().nodes();
                    let mut key_string_literal = None;
                    if let Some(&arg) = outer_arguments.first()
                        && ets_shim::ast::is_string_literal(arg)
                    {
                        key_string_literal = Some(arg);
                    }

                    // Extract the options expression (second argument of the outer call, if present)
                    let mut options = None;
                    if outer_arguments.len() >= 2 {
                        options = Some(outer_arguments[1]);
                    }

                    return Some(EffectServiceResult {
                        class_name,
                        self_type_node: type_arguments.nodes()[0],
                        options,
                        key_string_literal,
                    });
                }

                None
            },
        )
    }
}
