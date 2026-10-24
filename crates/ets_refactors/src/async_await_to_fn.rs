use std::rc::Rc;
use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_rewriter::Tracker;
use ets_shim::ast::{self, Kind, Node, NodeFactory, NodeFlags, TokenFlags};
use ets_shim::astnav;
use ets_shim::core::{P, alloc_str};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

use crate::*;

// go: internal/refactors/async_await_to_fn.go:12 AsyncAwaitToFn
pub static ASYNC_AWAIT_TO_FN: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "asyncAwaitToFn",
    description: "Convert async/await to Effect.fn",
    kind: "rewrite.effect.asyncAwaitToFn",
    run: run_async_await_to_fn,
});

// go: internal/refactors/async_await_to_fn.go:19 runAsyncAwaitToFn
// Rust: tsrs's `get_token_at_position` never returns nil, so Go's nil check drops.
pub(crate) fn run_async_await_to_fn(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    // Walk ancestor chain to find the nearest async function
    let Some(async_fn) = find_async_function(Some(token)) else {
        return Vec::new();
    };

    let effect_module_name = ets_typeparser::find_effect_module_identifier(ctx.source_file);

    let sf = ctx.source_file;
    let action = ctx.new_refactor_action(RefactorAction {
        description: "Rewrite to Effect.fn".to_string(),
        run: Box::new(move |tracker: &mut Tracker| {
            let new_decl = transform_async_to_effect_fn(tracker, async_fn, &effect_module_name);
            if let Some(new_decl) = new_decl {
                ast::set_parent_in_children(new_decl);
                // For function declarations, we replace the whole declaration with a variable statement
                if async_fn.kind() == Kind::FunctionDeclaration {
                    tracker.replace_node(sf, Some(async_fn), Some(new_decl), None);
                } else {
                    replace_expression_via_statement(tracker, sf, async_fn, new_decl);
                }
            }
        }),
    });
    let Some(mut action) = action else {
        return Vec::new();
    };

    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.asyncAwaitToFn");
    vec![action]
}

// go: internal/refactors/async_await_to_fn.go:57 transformAsyncToEffectFn
// transformAsyncToEffectFn transforms an async function to use Effect.fn.
pub(crate) fn transform_async_to_effect_fn(
    tracker: &Tracker,
    node: P<Node>,
    effect_module_name: &str,
) -> Option<P<Node>> {
    let body = ets_typeparser::get_function_like_body(node)?;

    // Transform await expressions to yield* Effect.promise(...)
    let module_name = effect_module_name.to_string();
    let transformed_body = transform_body_await_to_yield(
        tracker,
        body,
        Rc::new(move |t: &NodeFactory, expr: P<Node>| {
            build_yield_star_promise(t, expr, &module_name)
        }),
    );

    // Get the function name for the trace string
    let fn_name = ets_typeparser::get_function_like_name(node);

    // Build Effect.fn("name")(function*(params) { body })
    let effect_fn_call =
        build_effect_fn_call(tracker, node, transformed_body, effect_module_name, &fn_name);

    // Build declaration wrapping the fn call
    Some(build_fn_declaration(tracker, node, effect_fn_call, &fn_name))
}

// go: internal/refactors/async_await_to_fn.go:79 buildEffectFnCall
// buildEffectFnCall builds: Effect.fn("name")(function*(params) { body })
pub(crate) fn build_effect_fn_call(
    tracker: &NodeFactory,
    node: P<Node>,
    body: P<Node>,
    effect_module_name: &str,
    fn_name: &str,
) -> P<Node> {
    let block_body = if body.kind() == Kind::Block {
        body
    } else {
        // Expression body: wrap in { return <expr> }
        let return_stmt = tracker.new_return_statement(Some(body));
        tracker.new_block(tracker.new_node_list(vec![return_stmt]), true)
    };

    // Get type parameters and parameters from original function
    let mut type_params = None;
    let mut params = None;

    match node.kind() {
        Kind::ArrowFunction => {
            let af = node.as_arrow_function();
            type_params = clone_node_list(tracker, af.type_parameters());
            params = clone_node_list(tracker, af.parameters());
        }
        Kind::FunctionDeclaration => {
            let fd = node.as_function_declaration();
            type_params = clone_node_list(tracker, fd.type_parameters());
            params = clone_node_list(tracker, fd.parameters());
        }
        Kind::FunctionExpression => {
            let fe = node.as_function_expression();
            type_params = clone_node_list(tracker, fe.type_parameters());
            params = clone_node_list(tracker, fe.parameters());
        }
        _ => {}
    }

    if params.is_none() {
        params = Some(tracker.new_node_list(vec![]));
    }

    // function*(params) { ... }
    let gen_fn = tracker.new_function_expression(
        None,                                         // modifiers
        Some(tracker.new_token(Kind::AsteriskToken)), // asterisk (generator)
        None,                                         // name
        type_params,                                  // typeParameters
        params,                                       // parameters
        None,                                         // returnType
        None,                                         // fullSignature
        Some(block_body),
    );

    // Build Effect.fn or Effect.fn("name")
    let effect_id = tracker.new_identifier(alloc_str(effect_module_name));
    let fn_access = tracker.new_property_access_expression(
        effect_id,
        None,
        tracker.new_identifier("fn"),
        NodeFlags::None,
    );

    let fn_call_expr = if !fn_name.is_empty() {
        // Effect.fn("name")
        tracker.new_call_expression(
            fn_access,
            None,
            None,
            tracker.new_node_list(vec![
                tracker.new_string_literal(alloc_str(fn_name), TokenFlags::None),
            ]),
            NodeFlags::None,
        )
    } else {
        fn_access
    };

    // Effect.fn("name")(function*(params) { ... })
    tracker.new_call_expression(
        fn_call_expr,
        None,
        None,
        tracker.new_node_list(vec![gen_fn]),
        NodeFlags::None,
    )
}

// go: internal/refactors/async_await_to_fn.go:153 buildFnDeclaration
// buildFnDeclaration builds the appropriate declaration wrapping the Effect.fn call.
// For function declarations: const name = Effect.fn("name")(...)
// For expressions/arrows: just the Effect.fn("name")(...) expression
pub(crate) fn build_fn_declaration(
    tracker: &NodeFactory,
    node: P<Node>,
    effect_fn_call: P<Node>,
    _fn_name: &str,
) -> P<Node> {
    if node.kind() == Kind::FunctionDeclaration {
        let fd = node.as_function_declaration();
        let Some(name) = fd.name() else {
            return effect_fn_call;
        };

        // Build modifiers without async
        let modifiers = get_modifiers_without_async(tracker, node);

        // const name = Effect.fn("name")(...)
        let var_decl = tracker.new_variable_declaration(
            tracker.deep_clone_node(Some(name)).unwrap(),
            None, // exclamationToken
            None, // type
            Some(effect_fn_call),
        );
        let var_decl_list = tracker
            .new_variable_declaration_list(tracker.new_node_list(vec![var_decl]), NodeFlags::Const);
        return tracker.new_variable_statement(modifiers, var_decl_list);
    }

    // For arrow functions and function expressions, return the expression directly
    effect_fn_call
}
