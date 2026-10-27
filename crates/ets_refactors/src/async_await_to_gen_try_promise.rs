use ets_rewriter::NodeFactoryPaths;
use ets_typeparser::ModuleReference;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_rewriter::Tracker;
use ets_shim::ast::{
    self, Kind, ModifierFlags, Node, NodeFactory, NodeFlags, SourceFile, TokenFlags,
};
use ets_shim::astnav;
use ets_shim::core::{P, alloc_str};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

use crate::*;

// go: internal/refactors/async_await_to_gen_try_promise.go:14 AsyncAwaitToGenTryPromise
pub static ASYNC_AWAIT_TO_GEN_TRY_PROMISE: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "asyncAwaitToGenTryPromise",
    description: "Convert async/await to Effect.gen (with error handling)",
    kind: "rewrite.effect.asyncAwaitToGenTryPromise",
    run: run_async_await_to_gen_try_promise,
});

// go: internal/refactors/async_await_to_gen_try_promise.go:21 runAsyncAwaitToGenTryPromise
// Rust: tsrs's `get_token_at_position` never returns nil, so Go's nil check drops.
pub(crate) fn run_async_await_to_gen_try_promise(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    // Walk ancestor chain to find the nearest async function
    let Some(async_fn) = find_async_function(Some(token)) else {
        return Vec::new();
    };

    let effect_module_name = ets_typeparser::find_effect_module_reference(ctx.source_file);
    let data_module_name = ets_typeparser::find_module_reference(ctx.source_file, "Data");

    let sf = ctx.source_file;
    let action = ctx.new_refactor_action(RefactorAction {
        description: "Rewrite to Effect.gen with failures".to_string(),
        run: Box::new(move |tracker: &mut Tracker| {
            transform_async_to_effect_gen_try_promise(
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

    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.asyncAwaitToGenTryPromise");
    vec![action]
}

// go: internal/refactors/async_await_to_gen_try_promise.go:50 transformAsyncToEffectGenTryPromise
// Rust: the await callback runs inside a `'static` visitor closure (`TransformAwaitExpr`), so the error count and
// the error classes it collects are shared cells.
pub(crate) fn transform_async_to_effect_gen_try_promise(
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

    // Build Effect.gen(function*() { transformedBody })
    let effect_gen_call = build_effect_gen_call(tracker, transformed_body, effect_module_name);

    // Build non-async declaration wrapping the gen call
    let Some(new_decl) = build_non_async_declaration(tracker, node, effect_gen_call) else {
        return;
    };

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

// go: internal/refactors/async_await_to_gen_try_promise.go:112 findTopLevelStatement
// findTopLevelStatement walks up the parent chain from node until it finds
// a node whose parent is the SourceFile.
pub(crate) fn find_top_level_statement(node: P<Node>) -> P<Node> {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if parent.kind() == Kind::SourceFile {
            break;
        }
        current = parent;
    }
    current
}

// go: internal/refactors/async_await_to_gen_try_promise.go:121 buildYieldStarTryPromise
// buildYieldStarTryPromise builds: yield* Effect.tryPromise({ try: () => expr, catch: cause => new ErrorN({ cause }) })
pub(crate) fn build_yield_star_try_promise(
    tracker: &NodeFactory,
    expr: P<Node>,
    effect_module_name: &ModuleReference,
    error_name: &str,
) -> P<Node> {
    // try: () => expr
    let try_arrow = tracker.new_arrow_function(
        None,                                // modifiers
        None,                                // typeParameters
        Some(tracker.new_node_list(vec![])), // parameters (empty)
        None,                                // returnType
        None,                                // fullSignature
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        Some(expr),
    );
    let try_prop = tracker.new_property_assignment(
        None, // modifiers
        tracker.new_identifier("try"),
        None, // postfixToken
        None, // typeNode
        try_arrow,
    );

    // catch: cause => new ErrorN({ cause })
    let cause_param = tracker.new_parameter_declaration(
        None, // modifiers
        None, // dotDotDotToken
        tracker.new_identifier("cause"),
        None, // questionToken
        None, // typeNode
        None, // initializer
    );

    // new ErrorN({ cause })
    let cause_shorthand = tracker.new_shorthand_property_assignment(
        None, // modifiers
        tracker.new_identifier("cause"),
        None, // postfixToken
        None, // typeNode
        None, // equalsToken
        None, // objectAssignmentInitializer
    );
    let obj_literal = tracker.new_object_literal_expression(
        tracker.new_node_list(vec![cause_shorthand]),
        false, // not multiline
    );
    let new_error_expr = tracker.new_new_expression(
        tracker.new_identifier(alloc_str(error_name)),
        None, // typeArguments
        Some(tracker.new_node_list(vec![obj_literal])),
    );

    let catch_arrow = tracker.new_arrow_function(
        None, // modifiers
        None, // typeParameters
        Some(tracker.new_node_list(vec![cause_param])),
        None, // returnType
        None, // fullSignature
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        Some(new_error_expr),
    );
    let catch_prop = tracker.new_property_assignment(
        None, // modifiers
        tracker.new_identifier("catch"),
        None, // postfixToken
        None, // typeNode
        catch_arrow,
    );

    // { try: () => expr, catch: cause => new ErrorN({ cause }) }
    let options_obj = tracker.new_object_literal_expression(
        tracker.new_node_list(vec![try_prop, catch_prop]),
        false, // not multiline
    );

    // Effect.tryPromise({ ... })
    let try_promise_access =
        tracker.new_property_access_path(&effect_module_name.append(&["tryPromise"]));
    let try_promise_call = tracker.new_call_expression(
        try_promise_access,
        None,
        None,
        tracker.new_node_list(vec![options_obj]),
        NodeFlags::None,
    );

    // yield* Effect.tryPromise({ ... })
    tracker
        .new_yield_expression(Some(tracker.new_token(Kind::AsteriskToken)), Some(try_promise_call))
}

// go: internal/refactors/async_await_to_gen_try_promise.go:208 buildTaggedErrorClass
// buildTaggedErrorClass builds: class ErrorN extends Data.TaggedError("ErrorN")<{ cause: unknown }> {}
pub(crate) fn build_tagged_error_class(
    tracker: &NodeFactory,
    data_module_name: &ModuleReference,
    error_name: &str,
    is_exported: bool,
) -> P<Node> {
    // Data.TaggedError("ErrorN")
    let tagged_error_access =
        tracker.new_property_access_path(&data_module_name.append(&["TaggedError"]));
    let tagged_error_call = tracker.new_call_expression(
        tagged_error_access,
        None,
        None,
        tracker.new_node_list(vec![
            tracker.new_string_literal(alloc_str(error_name), TokenFlags::None),
        ]),
        NodeFlags::None,
    );

    // <{ cause: unknown }>
    let cause_prop_sig = tracker.new_property_signature_declaration(
        None, // modifiers
        tracker.new_identifier("cause"),
        None, // postfixToken
        Some(tracker.new_keyword_type_node(Kind::UnknownKeyword)),
        None, // initializer
    );
    let type_literal = tracker.new_type_literal_node(tracker.new_node_list(vec![cause_prop_sig]));

    // Data.TaggedError("ErrorN")<{ cause: unknown }>
    let expr_with_type_args = tracker.new_expression_with_type_arguments(
        tagged_error_call,
        Some(tracker.new_node_list(vec![type_literal])),
    );

    // extends clause
    let heritage_clause = tracker.new_heritage_clause(
        Kind::ExtendsKeyword,
        tracker.new_node_list(vec![expr_with_type_args]),
    );

    // Modifiers (export if needed)
    let mut modifiers = None;
    if is_exported {
        modifiers =
            Some(tracker.new_modifier_list(vec![tracker.new_modifier(Kind::ExportKeyword)]));
    }

    // class ErrorN extends Data.TaggedError("ErrorN")<{ cause: unknown }> {}
    tracker.new_class_declaration(
        modifiers,
        Some(tracker.new_identifier(alloc_str(error_name))),
        None, // typeParameters
        Some(tracker.new_node_list(vec![heritage_clause])),
        tracker.new_node_list(vec![]), // empty members
    )
}
