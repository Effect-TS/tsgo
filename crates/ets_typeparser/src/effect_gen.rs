// Package typeparser provides Effect type detection and parsing utilities.

use std::sync::Arc;

use ets_readonly::Slice;
use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/effect_gen.go:10 EffectGenCallResult
// EffectGenCallResult represents a parsed Effect.gen(...) call.
//
// Rust: Go's node pointer fields are `P<Node>`; the pipe arguments are the call's own argument list (`None` where
// Go's slice is nil).
pub struct EffectGenCallResult {
    pub(crate) call: P<Node>,
    pub(crate) effect_module: Option<P<Node>>, // Namespace receiver for Effect.gen; nil for named imports and local aliases.
    pub(crate) options_node: Option<P<Node>>,
    pub(crate) generator_function: P<Node>,
    pub(crate) body: Option<P<Node>>,
    pub(crate) pipe_arguments: Option<&'static [P<Node>]>,
}

impl EffectGenCallResult {
    // go: internal/typeparser/effect_gen.go:19 EffectGenCallResult.Call
    pub fn call(&self) -> P<Node> {
        self.call
    }

    // go: internal/typeparser/effect_gen.go:21 EffectGenCallResult.EffectModule
    pub fn effect_module(&self) -> Option<P<Node>> {
        self.effect_module
    }

    // go: internal/typeparser/effect_gen.go:23 EffectGenCallResult.OptionsNode
    pub fn options_node(&self) -> Option<P<Node>> {
        self.options_node
    }

    // go: internal/typeparser/effect_gen.go:25 EffectGenCallResult.GeneratorFunction
    pub fn generator_function(&self) -> P<Node> {
        self.generator_function
    }

    // go: internal/typeparser/effect_gen.go:27 EffectGenCallResult.Body
    pub fn body(&self) -> Option<P<Node>> {
        self.body
    }

    // go: internal/typeparser/effect_gen.go:29 EffectGenCallResult.PipeArguments
    pub fn pipe_arguments(&self) -> Slice<'_, P<Node>> {
        ets_readonly::new(self.pipe_arguments)
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_gen.go:35 TypeParser.EffectGenCall
    // EffectGenCall parses a node as Effect.gen(<generator>).
    // Returns nil when the node is not an Effect.gen call.
    pub fn effect_gen_call(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<Arc<EffectGenCallResult>> {
        let node = node.into()?;
        if node.kind() != Kind::CallExpression {
            return None;
        }

        cached(
            self,
            |l| &mut l.effect_gen_call,
            node,
            |tp| {
                let call = node.as_call_expression();
                let arguments = call.arguments().nodes();
                if arguments.is_empty() {
                    return None;
                }

                let (options_node, body_arg, pipe_args) = split_effect_fn_arguments(arguments);
                if !is_generator_function_node(body_arg) {
                    return None;
                }
                let gen_fn = body_arg.expect("a generator function node");

                let expr = call.expression();
                if !tp.is_node_reference_to_effect_module_api(expr, "gen") {
                    return None;
                }

                let mut effect_module = None;
                if expr.kind() == Kind::PropertyAccessExpression {
                    effect_module = Some(expr.as_property_access_expression().expression());
                }

                Some(Arc::new(EffectGenCallResult {
                    call: node,
                    effect_module,
                    options_node,
                    generator_function: gen_fn,
                    body: gen_fn.as_function_expression().body(),
                    pipe_arguments: pipe_args,
                }))
            },
        )
    }
}
