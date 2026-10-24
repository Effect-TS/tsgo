use ets_shim::ast::Node;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/extends_context_tag.go:8 ContextTagResult
// ContextTagResult holds the parsed result of a class extending Context.Tag.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ContextTagResult {
    pub(crate) class_name: P<Node>,     // The class name identifier
    pub(crate) self_type_node: P<Node>, // The Self type argument node (first type arg of the outer call)
    pub(crate) key_string_literal: Option<P<Node>>, // The key string literal from the inner call's first argument, or nil
}

impl ContextTagResult {
    // go: internal/typeparser/extends_context_tag.go:14 ContextTagResult.ClassName
    pub fn class_name(&self) -> P<Node> {
        self.class_name
    }

    // go: internal/typeparser/extends_context_tag.go:16 ContextTagResult.SelfTypeNode
    pub fn self_type_node(&self) -> P<Node> {
        self.self_type_node
    }

    // go: internal/typeparser/extends_context_tag.go:18 ContextTagResult.KeyStringLiteral
    pub fn key_string_literal(&self) -> Option<P<Node>> {
        self.key_string_literal
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/extends_context_tag.go:30 TypeParser.ExtendsContextTag
    // ExtendsContextTag checks if a class declaration extends Context.Tag("key")<Self, Shape>().
    // It detects the pattern:
    //
    //	class X extends Context.Tag("key")<X, Shape>() {}
    //
    // where the ExpressionWithTypeArguments.expression is a CallExpression (outer call)
    // that has type arguments <Self, Shape>, and whose own .expression is a CallExpression
    // (inner call: Context.Tag("key")), and the inner call's expression resolves to Context.Tag.
    //
    // Returns nil if the class does not extend Context.Tag.
    pub fn extends_context_tag(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<ContextTagResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_context_tag,
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

                    // The expression should be a CallExpression (the outer call with type arguments)
                    let outer_call_node = ewta.expression();
                    if !ets_shim::ast::is_call_expression(outer_call_node) {
                        continue;
                    }
                    let outer_call = outer_call_node.as_call_expression();

                    // The outer call must have type arguments (<Self, Shape>)
                    let Some(type_arguments) = outer_call.type_arguments() else {
                        continue;
                    };
                    if type_arguments.nodes().is_empty() {
                        continue;
                    }

                    // The outer call's expression should also be a CallExpression (the inner call: Context.Tag("key"))
                    let inner_call_node = outer_call.expression();
                    if !ets_shim::ast::is_call_expression(inner_call_node) {
                        continue;
                    }
                    let inner_call = inner_call_node.as_call_expression();

                    // Check if the inner call's expression resolves to Context.Tag
                    if !tp.is_node_reference_to_effect_context_module_api(
                        inner_call.expression(),
                        "Tag",
                    ) {
                        continue;
                    }

                    // Extract key string literal from inner call's first argument
                    let mut key_string_literal = None;
                    if let Some(&arg) = inner_call.arguments().nodes().first()
                        && ets_shim::ast::is_string_literal(arg)
                    {
                        key_string_literal = Some(arg);
                    }

                    return Some(ContextTagResult {
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
