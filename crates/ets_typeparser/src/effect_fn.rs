// Package typeparser provides Effect type detection and parsing utilities.

use std::sync::Arc;

use ets_readonly::Slice;
use ets_shim::ast::{Kind, Node};
use ets_shim::checker::{SignatureKind, Type};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/effect_fn.go:10 EffectFnVariant
// go: internal/typeparser/effect_fn.go:12 EffectFnVariantFn
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EffectFnVariant {
    Fn,
    FnUntraced,
    FnUntracedEager,
}

// go: internal/typeparser/effect_fn.go:10 EffectFnVariant
impl EffectFnVariant {
    /// Rust-only: the Go string value (`string(variant)`).
    pub fn as_str(self) -> &'static str {
        match self {
            EffectFnVariant::Fn => "fn",
            EffectFnVariant::FnUntraced => "fnUntraced",
            EffectFnVariant::FnUntracedEager => "fnUntracedEager",
        }
    }
}

// go: internal/typeparser/effect_fn.go:19 EffectFnCallResult
// EffectFnCallResult represents a parsed Effect.fn-family call.
//
// Rust: Go's `*ast.CallExpression`, `*ast.Expression` and `*ast.Node` fields are all `P<Node>`; the argument
// slices are the call's own argument list (`'static` like every parsed node list), `None` where Go's is nil.
pub struct EffectFnCallResult {
    pub(crate) call: P<Node>,
    pub(crate) variant: EffectFnVariant,
    pub(crate) effect_module: Option<P<Node>>,
    pub(crate) options_node: Option<P<Node>>,
    pub(crate) function_node: P<Node>, // ArrowFunction or FunctionExpression
    pub(crate) function_return_type: Lazy<Option<P<Type>>>,
    pub(crate) pipe_arguments: Option<&'static [P<Node>]>, // Transformation args after the body (may be empty/nil)
    pub(crate) pipe_args_out_type: Vec<Lazy<Option<P<Type>>>>,
    pub(crate) trace_expression: Option<P<Node>>, // The name string from curried Effect.fn("name")(...), or nil
}

impl EffectFnCallResult {
    // go: internal/typeparser/effect_fn.go:31 EffectFnCallResult.Call
    pub fn call(&self) -> P<Node> {
        self.call
    }

    // go: internal/typeparser/effect_fn.go:33 EffectFnCallResult.Variant
    pub fn variant(&self) -> EffectFnVariant {
        self.variant
    }

    // go: internal/typeparser/effect_fn.go:35 EffectFnCallResult.EffectModule
    pub fn effect_module(&self) -> Option<P<Node>> {
        self.effect_module
    }

    // go: internal/typeparser/effect_fn.go:37 EffectFnCallResult.OptionsNode
    pub fn options_node(&self) -> Option<P<Node>> {
        self.options_node
    }

    // go: internal/typeparser/effect_fn.go:39 EffectFnCallResult.FunctionNode
    pub fn function_node(&self) -> P<Node> {
        self.function_node
    }

    // go: internal/typeparser/effect_fn.go:41 EffectFnCallResult.FunctionReturnType
    // Rust: the lazy resolves through `tp` (see `Lazy`).
    pub fn function_return_type(&self, tp: &mut TypeParser<'_>) -> Option<P<Type>> {
        Lazy::get(Some(&self.function_return_type), tp)
    }

    // go: internal/typeparser/effect_fn.go:43 EffectFnCallResult.PipeArguments
    pub fn pipe_arguments(&self) -> Slice<'_, P<Node>> {
        ets_readonly::new(self.pipe_arguments)
    }

    // go: internal/typeparser/effect_fn.go:47 EffectFnCallResult.PipeArgsOutType
    // Rust: the lazies resolve through `tp` (see `Lazy`); Go returns a read-only view of the fresh slice, which
    // nothing else holds, so it is returned owned.
    pub fn pipe_args_out_type(&self, tp: &mut TypeParser<'_>) -> Vec<Option<P<Type>>> {
        let mut out_types = Vec::with_capacity(self.pipe_args_out_type.len());
        for out_type in &self.pipe_args_out_type {
            out_types.push(Lazy::get(Some(out_type), tp));
        }
        out_types
    }

    // go: internal/typeparser/effect_fn.go:55 EffectFnCallResult.TraceExpression
    pub fn trace_expression(&self) -> Option<P<Node>> {
        self.trace_expression
    }

    // go: internal/typeparser/effect_fn.go:57 EffectFnCallResult.IsGenerator
    pub fn is_generator(&self) -> bool {
        self.generator_function().is_some()
    }

    // go: internal/typeparser/effect_fn.go:61 EffectFnCallResult.GeneratorFunction
    pub fn generator_function(&self) -> Option<P<Node>> {
        if self.function_node.kind() != Kind::FunctionExpression {
            return None;
        }
        let f = self.function_node.as_function_expression();
        f.asterisk_token()?;
        Some(self.function_node)
    }

    // go: internal/typeparser/effect_fn.go:72 EffectFnCallResult.Body
    pub fn body(&self) -> Option<P<Node>> {
        match self.function_node.kind() {
            Kind::ArrowFunction => self.function_node.as_arrow_function().body(),
            Kind::FunctionExpression => self.function_node.as_function_expression().body(),
            _ => None,
        }
    }
}

// go: internal/typeparser/effect_fn.go:94 splitEffectFnArguments
pub(crate) fn split_effect_fn_arguments(
    args: &'static [P<Node>],
) -> (Option<P<Node>>, Option<P<Node>>, Option<&'static [P<Node>]>) {
    let mut start = 0;
    let mut options = None;
    if let Some(&first) = args.first()
        && first.kind() != Kind::ArrowFunction
        && first.kind() != Kind::FunctionExpression
    {
        options = Some(first);
        start = 1;
    }
    for i in start..args.len() {
        let arg = args[i];
        match arg.kind() {
            Kind::ArrowFunction | Kind::FunctionExpression => {
                if i + 1 < args.len() {
                    return (options, Some(arg), Some(&args[i + 1..]));
                }
                return (options, Some(arg), None);
            }
            _ => {}
        }
    }
    (options, None, None)
}

// go: internal/typeparser/effect_fn.go:120 isGeneratorFunctionNode
pub(crate) fn is_generator_function_node(node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if node.kind() != Kind::FunctionExpression {
        return false;
    }
    let f = node.as_function_expression();
    f.asterisk_token().is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_fn.go:128 TypeParser.buildEffectFnPipeArgsOutType
    #[expect(
        clippy::unused_self,
        reason = "a TypeParser method in Go; its lazies resolve through the parser they are read with"
    )]
    pub(crate) fn build_effect_fn_pipe_args_out_type(
        &self,
        call: P<Node>,
        trailing_start_index: usize,
        pipe_args: Option<&'static [P<Node>]>,
    ) -> Vec<Lazy<Option<P<Type>>>> {
        let pipe_args = pipe_args.unwrap_or_default();
        let mut out_types = Vec::with_capacity(pipe_args.len());
        for i in 0..pipe_args.len() {
            let arg_index = trailing_start_index + i;
            out_types
                .push(new_lazy(move |tp| tp.build_effect_fn_pipe_arg_out_type(call, arg_index)));
        }
        out_types
    }

    // go: internal/typeparser/effect_fn.go:139 TypeParser.buildEffectFnPipeArgOutType
    pub(crate) fn build_effect_fn_pipe_arg_out_type(
        &mut self,
        call: P<Node>,
        arg_index: usize,
    ) -> Option<P<Type>> {
        // Go: GetContextualTypeForArgumentAtIndex returns a type for every argument index.
        let contextual_type =
            self.checker.get_contextual_type_for_argument_at_index_exported(call, arg_index);
        let call_sigs =
            self.checker.get_signatures_of_type_exported(contextual_type, SignatureKind::Call);
        if call_sigs.is_empty() {
            return None;
        }
        Some(self.checker.get_return_type_of_signature_exported(call_sigs[0]))
    }

    // go: internal/typeparser/effect_fn.go:154 TypeParser.buildEffectFnFunctionReturnType
    pub(crate) fn build_effect_fn_function_return_type(
        &mut self,
        call: P<Node>,
        trailing_start_index: usize,
        pipe_args: Option<&'static [P<Node>]>,
    ) -> Option<P<Type>> {
        let pipe_args = pipe_args.unwrap_or_default();
        if pipe_args.is_empty() {
            let fn_type = self.get_type_at_location(call)?;
            let call_sigs =
                self.checker.get_signatures_of_type_exported(fn_type, SignatureKind::Call);
            if call_sigs.is_empty() {
                return None;
            }
            return Some(self.checker.get_return_type_of_signature_exported(call_sigs[0]));
        }

        // Go: GetResolvedSignature never returns nil.
        let resolved = self.checker.get_resolved_signature_exported(call);
        let params = resolved.parameters();
        if trailing_start_index >= params.len() {
            return None;
        }
        let first_pipe_param_type = self
            .checker
            .get_type_of_symbol_at_location(params[trailing_start_index], Some(pipe_args[0]))?;
        let first_pipe_call_sigs = self
            .checker
            .get_signatures_of_type_exported(first_pipe_param_type, SignatureKind::Call);
        if first_pipe_call_sigs.is_empty() {
            return None;
        }
        let pipe_input_params = first_pipe_call_sigs[0].parameters();
        if pipe_input_params.is_empty() {
            return None;
        }
        self.checker.get_type_of_symbol_at_location(pipe_input_params[0], Some(pipe_args[0]))
    }

    // go: internal/typeparser/effect_fn.go:196 TypeParser.EffectFnCall
    // EffectFnCall parses a node as an Effect.fn-family call.
    // It supports fn, fnUntraced, and fnUntracedEager, both regular and generator forms.
    pub fn effect_fn_call(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<Arc<EffectFnCallResult>> {
        let node = node.into()?;
        if node.kind() != Kind::CallExpression {
            return None;
        }

        cached(
            self,
            |l| &mut l.effect_fn_call,
            node,
            |tp| {
                let call = node.as_call_expression();
                let arguments = call.arguments().nodes();
                if arguments.is_empty() {
                    return None;
                }

                let (options_node, body_arg, pipe_args) = split_effect_fn_arguments(arguments);
                let body_arg = body_arg?;
                let trailing_start_index = arguments.len() - pipe_args.unwrap_or_default().len();

                // Determine the expression to check for Effect.fn reference.
                // For curried calls like Effect.fn("name")(regularFn), call.Expression is a CallExpression.
                // For direct calls like Effect.fn(regularFn), call.Expression is a PropertyAccessExpression.
                let expr = call.expression();

                let expression_to_check;
                let mut trace_expression = None;

                if expr.kind() == Kind::CallExpression {
                    let inner_call = expr.as_call_expression();
                    expression_to_check = inner_call.expression();

                    // Extract trace expression from curried form: Effect.fn("name")(...)
                    if let Some(&first) = inner_call.arguments().nodes().first() {
                        trace_expression = Some(first);
                    }
                } else {
                    expression_to_check = expr;
                }

                let variant = if tp
                    .is_node_reference_to_effect_module_api(expression_to_check, "fn")
                {
                    EffectFnVariant::Fn
                } else if tp
                    .is_node_reference_to_effect_module_api(expression_to_check, "fnUntraced")
                {
                    if trace_expression.is_some() {
                        return None;
                    }
                    EffectFnVariant::FnUntraced
                } else if tp
                    .is_node_reference_to_effect_module_api(expression_to_check, "fnUntracedEager")
                {
                    if trace_expression.is_some() {
                        return None;
                    }
                    EffectFnVariant::FnUntracedEager
                } else {
                    return None;
                };

                let mut effect_module = None;
                if expression_to_check.kind() == Kind::PropertyAccessExpression {
                    let property_access = expression_to_check.as_property_access_expression();
                    effect_module = Some(property_access.expression());
                }

                Some(Arc::new(EffectFnCallResult {
                    call: node,
                    variant,
                    effect_module,
                    options_node,
                    function_node: body_arg,
                    function_return_type: new_lazy(move |tp| {
                        tp.build_effect_fn_function_return_type(
                            node,
                            trailing_start_index,
                            pipe_args,
                        )
                    }),
                    pipe_arguments: pipe_args,
                    pipe_args_out_type: tp.build_effect_fn_pipe_args_out_type(
                        node,
                        trailing_start_index,
                        pipe_args,
                    ),
                    trace_expression,
                }))
            },
        )
    }
}
