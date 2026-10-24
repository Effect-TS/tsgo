use std::rc::Rc;
use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_rewriter::Tracker;
use ets_shim::ast::{
    self, Kind, ModifierFlags, ModifierList, Node, NodeFactory, NodeFlags, NodeList, NodeVisitor,
    NodeVisitorHooks, SourceFile,
};
use ets_shim::astnav;
use ets_shim::core::{P, alloc_str};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

use crate::*;

// go: internal/refactors/async_await_to_gen.go:12 AsyncAwaitToGen
pub static ASYNC_AWAIT_TO_GEN: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "asyncAwaitToGen",
    description: "Convert async/await to Effect.gen",
    kind: "rewrite.effect.asyncAwaitToGen",
    run: run_async_await_to_gen,
});

// go: internal/refactors/async_await_to_gen.go:19 runAsyncAwaitToGen
// Rust: tsrs's `get_token_at_position` never returns nil, so Go's nil check drops.
pub(crate) fn run_async_await_to_gen(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    // Walk ancestor chain to find the nearest async function
    let Some(async_fn) = find_async_function(Some(token)) else {
        return Vec::new();
    };

    let effect_module_name = ets_typeparser::find_effect_module_identifier(ctx.source_file);

    let sf = ctx.source_file;
    let action = ctx.new_refactor_action(RefactorAction {
        description: "Rewrite to Effect.gen".to_string(),
        run: Box::new(move |tracker: &mut Tracker| {
            let new_decl = transform_async_to_effect_gen(tracker, async_fn, &effect_module_name);
            if let Some(new_decl) = new_decl {
                ast::set_parent_in_children(new_decl);
                // For FunctionDeclaration (a statement), replace directly.
                // For FunctionExpression/ArrowFunction (not statements), find the
                // containing statement and replace at that level to avoid a printer
                // assertion failure with complex synthetic trees in expression context.
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

    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.asyncAwaitToGen");
    vec![action]
}

// go: internal/refactors/async_await_to_gen.go:64 replaceExpressionViaStatement
// replaceExpressionViaStatement replaces an expression node that is the sole
// initializer of a VariableStatement by rebuilding and replacing the entire
// statement. This avoids a printer assertion failure that occurs when replacing
// non-statement nodes with complex synthetic trees. Any other expression is
// replaced in place.
//
// Rust: Go reads `varDecl.Parent.Parent` without nil checks (a parsed declaration always has both parents).
pub(crate) fn replace_expression_via_statement(
    tracker: &mut Tracker,
    sf: P<SourceFile>,
    old_expr: P<Node>,
    new_expr: P<Node>,
) {
    let var_decl = old_expr.parent();
    let Some(var_decl) = var_decl.filter(|var_decl| {
        let list = var_decl.parent().expect("a variable declaration has a parent");
        var_decl.kind() == Kind::VariableDeclaration
            && var_decl.initializer() == Some(old_expr)
            && list.kind() == Kind::VariableDeclarationList
            && list.as_variable_declaration_list().declarations().nodes().len() == 1
            && list.parent().expect("a declaration list has a parent").kind()
                == Kind::VariableStatement
    }) else {
        tracker.replace_node(sf, Some(old_expr), Some(new_expr), None);
        return;
    };
    let var_decl_list = var_decl.parent().unwrap();
    let var_stmt = var_decl_list.parent().unwrap();

    // Rebuild the VariableDeclaration with new initializer
    let vd = var_decl.as_variable_declaration();
    let name = tracker.deep_clone_node(Some(vd.name())).unwrap();
    let type_node = tracker.deep_clone_node(vd.type_());
    let new_var_decl = tracker.new_variable_declaration(name, None, type_node, Some(new_expr));

    // Rebuild the VariableDeclarationList
    let new_var_decl_list = tracker.new_variable_declaration_list(
        tracker.new_node_list(vec![new_var_decl]),
        var_decl_list.flags() & NodeFlags::BlockScoped,
    );

    // Rebuild the VariableStatement with cloned modifiers
    let vs = var_stmt.as_variable_statement();
    let mut mods = None;
    if let Some(modifiers) = vs.modifiers() {
        let mut mod_nodes = Vec::new();
        for &m in modifiers.nodes() {
            mod_nodes.push(tracker.new_modifier(m.kind()));
        }
        if !mod_nodes.is_empty() {
            mods = Some(tracker.new_modifier_list(mod_nodes));
        }
    }
    let new_var_stmt = tracker.new_variable_statement(mods, new_var_decl_list);

    ast::set_parent_in_children(new_var_stmt);
    tracker.replace_node(sf, Some(var_stmt), Some(new_var_stmt), None);
}

// go: internal/refactors/async_await_to_gen.go:109 findParentOfKind
// findParentOfKind walks up the parent chain to find a node of the given kind.
pub(crate) fn find_parent_of_kind(node: P<Node>, kind: Kind) -> Option<P<Node>> {
    let mut n = node.parent();
    while let Some(current) = n {
        if current.kind() == kind {
            return Some(current);
        }
        n = current.parent();
    }
    None
}

// go: internal/refactors/async_await_to_gen.go:120 findAsyncFunction
// findAsyncFunction walks the ancestor chain from token to find the nearest
// async function declaration, function expression, or arrow function.
pub(crate) fn find_async_function(token: Option<P<Node>>) -> Option<P<Node>> {
    let mut node = token;
    while let Some(n) = node {
        match n.kind() {
            Kind::FunctionDeclaration | Kind::FunctionExpression | Kind::ArrowFunction => {
                if ast::get_combined_modifier_flags(n).intersects(ModifierFlags::Async)
                    && ets_typeparser::get_function_like_body(n).is_some()
                {
                    return Some(n);
                }
                // If we hit a non-async function boundary, stop looking
                return None;
            }
            _ => {}
        }
        node = n.parent();
    }
    None
}

// go: internal/refactors/async_await_to_gen.go:137 transformAsyncToEffectGen
// transformAsyncToEffectGen transforms an async function to use Effect.gen.
pub(crate) fn transform_async_to_effect_gen(
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

    // Build Effect.gen(function*() { transformedBody })
    let effect_gen_call = build_effect_gen_call(tracker, transformed_body, effect_module_name);

    // Build non-async declaration wrapping the gen call
    build_non_async_declaration(tracker, node, effect_gen_call)
}

// go: internal/refactors/async_await_to_gen.go:156 TransformAwaitExpr
// TransformAwaitExpr is the callback type for transforming await expressions.
//
// Rust: the callback runs inside tsrs's node visitor, whose callback is a `'static` closure that cannot borrow the
// tracker, so it gets the tracker's factory (all Go's callbacks use the tracker only as a factory) and is shared.
pub type TransformAwaitExpr = Rc<dyn Fn(&NodeFactory, P<Node>) -> P<Node>>;

// go: internal/refactors/async_await_to_gen.go:160 transformBodyAwaitToYield
// transformBodyAwaitToYield replaces all AwaitExpression nodes in the body
// with the result of onAwait. Returns a deep-cloned, fully synthetic tree.
pub(crate) fn transform_body_await_to_yield(
    tracker: &NodeFactory,
    body: P<Node>,
    on_await: TransformAwaitExpr,
) -> P<Node> {
    // If no await expressions, just deep-clone the body
    if !contains_await_expression(body) {
        return tracker.deep_clone_node(Some(body)).unwrap();
    }

    // Visit the original body to transform await → yield*
    let factory = tracker.clone();
    let visit_fn = move |v: &mut NodeVisitor, n: P<Node>| -> Option<P<Node>> {
        if n.kind() == Kind::AwaitExpression {
            let await_expr = n.as_await_expression();
            let visited_inner = v
                .visit_node(Some(await_expr.expression()))
                .expect("the visitor keeps every expression");
            return Some(on_await(&factory, visited_inner));
        }
        v.visit_each_child(Some(n))
    };
    let mut v = ast::new_node_visitor(
        Some(Rc::new(visit_fn)),
        Some(tracker.clone()),
        NodeVisitorHooks::default(),
    );

    let visited = v.visit_node(Some(body));

    // Deep-clone the result to make all nodes synthetic
    let result = tracker.deep_clone_node(visited).unwrap();
    ast::set_parent_in_children(result);
    result
}

// go: internal/refactors/async_await_to_gen.go:187 containsAwaitExpression
// containsAwaitExpression checks if a node tree contains any AwaitExpression.
pub(crate) fn contains_await_expression(node: P<Node>) -> bool {
    if node.kind() == Kind::AwaitExpression {
        return true;
    }
    let mut found = false;
    node.for_each_child(&mut |child| {
        if contains_await_expression(child) {
            found = true;
            return true; // stop
        }
        false
    });
    found
}

// go: internal/refactors/async_await_to_gen.go:203 buildYieldStarPromise
// buildYieldStarPromise builds: yield* Effect.promise(() => expr)
pub(crate) fn build_yield_star_promise(
    tracker: &NodeFactory,
    expr: P<Node>,
    effect_module_name: &str,
) -> P<Node> {
    // () => expr
    let arrow_fn = tracker.new_arrow_function(
        None,                                // modifiers
        None,                                // typeParameters
        Some(tracker.new_node_list(vec![])), // parameters (empty)
        None,                                // returnType
        None,                                // fullSignature
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        Some(expr),
    );

    // Effect.promise(() => expr)
    let effect_id = tracker.new_identifier(alloc_str(effect_module_name));
    let promise_access = tracker.new_property_access_expression(
        effect_id,
        None,
        tracker.new_identifier("promise"),
        NodeFlags::None,
    );
    let promise_call = tracker.new_call_expression(
        promise_access,
        None,
        None,
        tracker.new_node_list(vec![arrow_fn]),
        NodeFlags::None,
    );

    // yield* Effect.promise(() => expr)
    tracker.new_yield_expression(Some(tracker.new_token(Kind::AsteriskToken)), Some(promise_call))
}

// go: internal/refactors/async_await_to_gen.go:234 buildEffectGenCall
// buildEffectGenCall builds: Effect.gen(function*() { body })
pub(crate) fn build_effect_gen_call(
    tracker: &NodeFactory,
    body: P<Node>,
    effect_module_name: &str,
) -> P<Node> {
    let block_body = if body.kind() == Kind::Block {
        body
    } else {
        // Expression body: wrap in { return <expr> }
        let return_stmt = tracker.new_return_statement(Some(body));
        tracker.new_block(tracker.new_node_list(vec![return_stmt]), true)
    };

    // function*() { ... }
    let gen_fn = tracker.new_function_expression(
        None,                                         // modifiers
        Some(tracker.new_token(Kind::AsteriskToken)), // asterisk (generator)
        None,                                         // name
        None,                                         // typeParameters
        Some(tracker.new_node_list(vec![])),          // parameters (empty)
        None,                                         // returnType
        None,                                         // fullSignature
        Some(block_body),
    );

    // Effect.gen(function*() { ... })
    let effect_id = tracker.new_identifier(alloc_str(effect_module_name));
    let gen_access = tracker.new_property_access_expression(
        effect_id,
        None,
        tracker.new_identifier("gen"),
        NodeFlags::None,
    );
    tracker.new_call_expression(
        gen_access,
        None,
        None,
        tracker.new_node_list(vec![gen_fn]),
        NodeFlags::None,
    )
}

// go: internal/refactors/async_await_to_gen.go:270 buildNonAsyncDeclaration
// buildNonAsyncDeclaration builds a new function declaration/expression/arrow
// without the async modifier, with the given effectGenCall as the new body.
pub(crate) fn build_non_async_declaration(
    tracker: &NodeFactory,
    node: P<Node>,
    effect_gen_call: P<Node>,
) -> Option<P<Node>> {
    let modifiers = get_modifiers_without_async(tracker, node);

    match node.kind() {
        Kind::ArrowFunction => {
            let af = node.as_arrow_function();
            Some(tracker.new_arrow_function(
                modifiers,
                clone_node_list(tracker, af.type_parameters()),
                clone_node_list(tracker, af.parameters()),
                None, // returnType
                None, // fullSignature
                Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
                Some(effect_gen_call),
            ))
        }

        Kind::FunctionDeclaration => {
            let fd = node.as_function_declaration();
            let return_stmt = tracker.new_return_statement(Some(effect_gen_call));
            let new_body = tracker.new_block(tracker.new_node_list(vec![return_stmt]), true);
            let name = tracker.deep_clone_node(fd.name());
            Some(tracker.new_function_declaration(
                modifiers,
                None, // asteriskToken
                name,
                clone_node_list(tracker, fd.type_parameters()),
                clone_node_list(tracker, fd.parameters()),
                None, // returnType
                None, // fullSignature
                Some(new_body),
            ))
        }

        Kind::FunctionExpression => {
            let fe = node.as_function_expression();
            let return_stmt = tracker.new_return_statement(Some(effect_gen_call));
            let new_body = tracker.new_block(tracker.new_node_list(vec![return_stmt]), true);
            let name = tracker.deep_clone_node(fe.name());
            Some(tracker.new_function_expression(
                modifiers,
                None, // asteriskToken
                name,
                clone_node_list(tracker, fe.type_parameters()),
                clone_node_list(tracker, fe.parameters()),
                None, // returnType
                None, // fullSignature
                Some(new_body),
            ))
        }

        _ => None,
    }
}

// go: internal/refactors/async_await_to_gen.go:329 getModifiersWithoutAsync
// getModifiersWithoutAsync returns a new modifier list with the async keyword removed.
pub(crate) fn get_modifiers_without_async(
    tracker: &NodeFactory,
    node: P<Node>,
) -> Option<P<ModifierList>> {
    let src_modifiers = match node.kind() {
        Kind::FunctionDeclaration => node.as_function_declaration().modifiers(),
        Kind::FunctionExpression => node.as_function_expression().modifiers(),
        Kind::ArrowFunction => node.as_arrow_function().modifiers(),
        _ => None,
    }?;

    let mut mod_nodes = Vec::new();
    for &m in src_modifiers.nodes() {
        if m.kind() == Kind::AsyncKeyword {
            continue;
        }
        mod_nodes.push(tracker.new_modifier(m.kind()));
    }

    if mod_nodes.is_empty() {
        return None;
    }
    Some(tracker.new_modifier_list(mod_nodes))
}

// go: internal/refactors/async_await_to_gen.go:359 cloneNodeList
// cloneNodeList deep-clones a NodeList if non-nil.
pub(crate) fn clone_node_list(
    tracker: &NodeFactory,
    list: Option<P<NodeList>>,
) -> Option<P<NodeList>> {
    let list = list?;
    if list.nodes().is_empty() {
        return Some(tracker.new_node_list(vec![]));
    }
    let cloned: Vec<P<Node>> =
        list.nodes().iter().map(|&n| tracker.deep_clone_node(Some(n)).unwrap()).collect();
    Some(tracker.new_node_list(cloned))
}
