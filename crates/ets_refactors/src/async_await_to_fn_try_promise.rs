use ets_typeparser::ModuleReference;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_rewriter::Tracker;
use ets_shim::ast::{self, Kind, ModifierFlags, Node, NodeFactory, SourceFile};
use ets_shim::astnav;
use ets_shim::core::P;
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

use crate::*;

// go: internal/refactors/async_await_to_fn_try_promise.go:14 AsyncAwaitToFnTryPromise 046582d5c1253bf5
pub static ASYNC_AWAIT_TO_FN_TRY_PROMISE: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "asyncAwaitToFnTryPromise",
    description: "Convert async/await to Effect.fn (with error handling)",
    kind: "rewrite.effect.asyncAwaitToFnTryPromise",
    run: run_async_await_to_fn_try_promise,
});

// go: internal/refactors/async_await_to_fn_try_promise.go:21 runAsyncAwaitToFnTryPromise a05ec2b5707860c2
// Rust: tsrs's `get_token_at_position` never returns nil, so Go's nil check drops.
pub(crate) fn run_async_await_to_fn_try_promise(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    // Walk ancestor chain to find the nearest async function
    let Some(async_fn) = find_async_function(Some(token)) else {
        return Vec::new();
    };

    let effect_module_name = ets_typeparser::find_effect_module_reference(ctx.source_file);
    let data_module_name = ets_typeparser::find_module_reference(ctx.source_file, "Data");

    let sf = ctx.source_file;
    let action = ctx.new_refactor_action(RefactorAction {
        description: "Rewrite to Effect.fn with failures".to_string(),
        run: Box::new(move |tracker: &mut Tracker| {
            transform_async_to_effect_fn_try_promise(
                tracker,
                sf,
                async_fn,
                &effect_module_name,
                &data_module_name,
            );
        }),
    });
    let Some(mut action) = action else {
        return Vec::new();
    };

    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.asyncAwaitToFnTryPromise");
    vec![action]
}

// go: internal/refactors/async_await_to_fn_try_promise.go:50 transformAsyncToEffectFnTryPromise c4d5cca8adbf57c5
// Rust: the await callback runs inside a `'static` visitor closure (`TransformAwaitExpr`), so the error count and
// the error classes it collects are shared cells. `build_fn_declaration` never returns nil, so Go's nil check
// after it drops.
pub(crate) fn transform_async_to_effect_fn_try_promise(
    tracker: &mut Tracker,
    sf: P<SourceFile>,
    node: P<Node>,
    effect_module_name: &ModuleReference,
    data_module_name: &ModuleReference,
) {
    let Some(body) = ets_typeparser::get_function_like_body(node) else {
        return;
    };

    // Track error classes to generate
    let error_count = Rc::new(Cell::new(0i64));
    let error_classes: Rc<RefCell<Vec<P<Node>>>> = Rc::new(RefCell::new(Vec::new()));

    // Determine if function is exported (error classes should match)
    let mut is_exported = ast::get_combined_modifier_flags(node).intersects(ModifierFlags::Export);
    // Also check parent variable statement for export
    if !is_exported && let Some(var_stmt) = find_parent_of_kind(node, Kind::VariableStatement) {
        is_exported = ast::get_combined_modifier_flags(var_stmt).intersects(ModifierFlags::Export);
    }

    // Transform await expressions to yield* Effect.tryPromise(...)
    let transformed_body = {
        let error_count = Rc::clone(&error_count);
        let error_classes = Rc::clone(&error_classes);
        let data_module_name = data_module_name.clone();
        let effect_module_name = effect_module_name.clone();
        transform_body_await_to_yield(
            tracker,
            body,
            Rc::new(move |t: &NodeFactory, expr: P<Node>| {
                error_count.set(error_count.get() + 1);
                let error_name = format!("Error{}", error_count.get());

                // Build the error class declaration
                let error_class =
                    build_tagged_error_class(t, &data_module_name, &error_name, is_exported);
                error_classes.borrow_mut().push(error_class);

                build_yield_star_try_promise(t, expr, &effect_module_name, &error_name)
            }),
        )
    };

    // Get the function name for the trace string
    let fn_name = ets_typeparser::get_function_like_name(node);

    // Build Effect.fn("name")(function*(params) { body })
    let effect_fn_call =
        build_effect_fn_call(tracker, node, transformed_body, effect_module_name, &fn_name);

    // Build declaration wrapping the fn call
    let new_decl = build_fn_declaration(tracker, node, effect_fn_call, &fn_name);

    ast::set_parent_in_children(new_decl);

    // Find the top-level statement containing the function (for inserting error classes before it)
    let top_level_stmt = find_top_level_statement(node);

    // Insert error classes before the top-level statement
    for error_class in error_classes.take() {
        ast::set_parent_in_children(error_class);
        tracker.insert_node_before(
            sf,
            Some(top_level_stmt),
            Some(error_class),
            true,
            ets_rewriter::LEADING_TRIVIA_OPTION_NONE,
        );
    }

    // Replace the function
    if node.kind() == Kind::FunctionDeclaration {
        tracker.replace_node(sf, Some(node), Some(new_decl), None);
    } else {
        replace_expression_via_statement(tracker, sf, node, new_decl);
    }
}
