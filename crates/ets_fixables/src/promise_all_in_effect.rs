use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rules::PromiseMethod;
use ets_shim::P;
use ets_shim::ast::{
    self, Kind, ModifierFlags, Node, NodeFactory, NodeFlags, Symbol, SymbolFlags, TokenFlags,
};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::{
    LazyExpressionFlags, PipingFlow, PipingFlowSubject, PipingFlowTransformation,
    TransformationKind, TypeParser,
};
use rustc_hash::FxHashSet;

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::X_0_STARTS_EVERY_PROMISE_AT_ONCE_WITH_NO_CONCURRENCY_BOUND_AND_THEY_KEEP_RUNNING_WHEN_THE_FIBER_IS_INTERRUPTED_USE_1_WITH_A_CONCURRENCY_OPTION_INSTEAD_EFFECT_PROMISE_ALL_IN_EFFECT;

// go: internal/fixables/promise_all_in_effect.go:13 PromiseAllInEffectFix 83aa36f3ff8d6323
pub static PROMISE_ALL_IN_EFFECT_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "promiseAllInEffect",
    description: "Replace with Effect.forEach",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["promiseAllInEffect_fix"],
    run: run_promise_all_in_effect_fix,
});

// go: internal/fixables/promise_all_in_effect.go:25 runPromiseAllInEffectFix d777fc0d0c1e09fe
// Only Promise.all is fixed: the
// success type of Promise.allSettled differs from any Effect.all mode.
//
// Rust: Go's `reported` map (only `true` values) is a set. Go's predicate closures take the parser as an argument
// (the flow matchers pass it in).
fn run_promise_all_in_effect_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let mut reported: FxHashSet<P<Node>> = FxHashSet::default();
    for m in ets_rules::analyze_promise_all_in_effect(&mut ctx.type_parser, Some(sf)) {
        if m.method != PromiseMethod::All
            || !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location)
        {
            continue;
        }
        reported.insert(m.flow.transformations().at(m.index).callee());
    }
    if reported.is_empty() {
        return Vec::new();
    }

    fn no_type_arguments(transformation: &PipingFlowTransformation) -> bool {
        transformation.type_arguments().is_none_or(|arguments| arguments.nodes().is_empty())
    }
    let is_sync_concise_unannotated_thunk =
        |_: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
            let Some(node) = subject.node().map(ast::skip_parentheses) else {
                return false;
            };
            if node.kind() != Kind::ArrowFunction {
                return false;
            }
            let thunk = ets_typeparser::parse_lazy_expression(node, LazyExpressionFlags::None);
            thunk.is_some_and(|thunk| {
                !ast::is_block(thunk.body())
                    && thunk.params().len() <= 1
                    && node.type_node().is_none()
            })
        };
    let is_promise_constructor =
        |tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
            transformation.kind() == TransformationKind::Call
                && transformation.args().len() == 0
                && no_type_arguments(transformation)
                && (tp.is_node_reference_to_effect_module_api(transformation.callee(), "promise")
                    || tp.is_node_reference_to_effect_module_api(
                        transformation.callee(),
                        "tryPromise",
                    ))
        };
    // Effect.forEach passes (item, index), so the callback may declare at most
    // two plain parameters. A `this` pseudo-parameter has no arrow form.
    let is_inline_callback = |_: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
        let Some(node) = subject.node().map(ast::skip_parentheses) else {
            return false;
        };
        if node.kind() != Kind::ArrowFunction && node.kind() != Kind::FunctionExpression {
            return false;
        }
        if node.kind() == Kind::FunctionExpression
            && node.as_function_expression().asterisk_token().is_some()
        {
            return false;
        }
        if !node.type_parameters().is_empty() {
            return false;
        }
        let parameters = node.parameters();
        if parameters.len() > 2 {
            return false;
        }
        for &parameter in parameters {
            if parameter.as_parameter_declaration().dot_dot_dot_token().is_some()
                || ast::is_this_parameter(parameter)
            {
                return false;
            }
        }
        true
    };
    let is_array_map = |tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
        let callee = transformation.callee();
        transformation.kind() == TransformationKind::Call
            && transformation.args().len() == 0
            && no_type_arguments(transformation)
            && ast::is_property_access_expression(callee)
            && callee.as_property_access_expression().question_dot_token().is_none()
            && callee.name().is_some_and(|name| name.text() == "map")
            && {
                let receiver_type = tp.get_type_at_location(callee.expression());
                tp.array_type(receiver_type).is_some()
            }
    };
    let is_reported_promise_all =
        |_: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
            transformation.kind() == TransformationKind::Call
                && no_type_arguments(transformation)
                && reported.contains(&transformation.callee())
        };

    let flows = ctx.type_parser.piping_flows(sf, true);
    for outer in flows.iter().flat_map(|flows| flows.iter()) {
        if !PipingFlow::matches_prefix(
            Some(outer),
            &mut ctx.type_parser,
            &is_sync_concise_unannotated_thunk,
            &[&is_promise_constructor],
        ) {
            continue;
        }
        let thunk = ast::skip_parentheses(outer.subject().node().unwrap());
        let body = thunk.body();
        let inner = ctx.type_parser.longest_piping_flow_at(body, true);
        let Some(inner) = inner else {
            continue;
        };
        if Some(inner.node()) != body.map(ast::skip_parentheses)
            || !PipingFlow::matches_exactly(
                Some(&inner),
                &mut ctx.type_parser,
                &is_inline_callback,
                &[&is_array_map, &is_reported_promise_all],
            )
        {
            continue;
        }
        let callback = ast::skip_parentheses(inner.subject().node().unwrap());
        let map_step = inner.transformations().at(0);
        let receiver = map_step.callee().as_property_access_expression().expression();
        // Each callback result becomes the per-item thunk's result, which
        // Effect.promise and Effect.tryPromise require to be a promise;
        // Promise.all also accepts plain values and unions with them.
        let map_out_type = map_step.out_type(&mut ctx.type_parser);
        let element_type = ctx.type_parser.array_type(map_out_type);
        if ctx.type_parser.promise_type(element_type).is_none() {
            continue;
        }
        let constructor_callee = outer.transformations().at(0).callee();
        if !promise_all_callback_moves_safely(ctx, thunk, callback, receiver, constructor_callee) {
            continue;
        }

        let member = effect_module_member("forEach");
        let mut qualifier: Option<P<Node>> = None;
        let mut target: Option<ModuleMemberReference> = None;
        if ast::is_property_access_expression(constructor_callee) {
            let expression = constructor_callee.as_property_access_expression().expression();
            if ctx.type_parser.is_expression_effect_module(expression) {
                let expression_type = ctx.type_parser.get_type_at_location(expression);
                if type_has_member(ctx, expression_type, &member) {
                    qualifier = Some(expression);
                }
            }
        }
        if qualifier.is_none() {
            target = imported_module_member(
                ctx,
                constructor_callee,
                &ets_typeparser::find_effect_module_reference(Some(sf)),
                &member,
            );
            if target.is_none() {
                continue;
            }
        }

        let outer = outer.clone();
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.forEach".to_string(),
            run: Box::new(move |tracker| {
                let for_each = if let Some(qualifier) = qualifier {
                    tracker.new_property_access_expression(
                        tracker.deep_clone_node(Some(qualifier)).unwrap(),
                        None,
                        tracker.new_identifier("forEach"),
                        NodeFlags::None,
                    )
                } else {
                    target.as_ref().unwrap().expression(tracker, &member)
                };
                let arguments = vec![
                    tracker.deep_clone_node(Some(receiver)).unwrap(),
                    new_promise_all_item_callback(tracker, thunk, callback, constructor_callee),
                    tracker.new_object_literal_expression(
                        tracker.new_node_list(vec![tracker.new_property_assignment(
                            None,
                            tracker.new_identifier("concurrency"),
                            None,
                            None,
                            tracker.new_string_literal("unbounded", TokenFlags::None),
                        )]),
                        false,
                    ),
                ];
                let replacement = tracker.new_call_expression(
                    for_each,
                    None,
                    None,
                    tracker.new_node_list(arguments),
                    NodeFlags::None,
                );
                tracker.replace_piping_flow_prefix(Some(sf), Some(&outer), 1, Some(replacement));
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/promise_all_in_effect.go:167 promiseAllCallbackMovesSafely e2b9ebc335b95560
fn promise_all_callback_moves_safely(
    ctx: &mut Context<'_, '_>,
    thunk: P<Node>,
    callback: P<Node>,
    receiver: P<Node>,
    constructor_callee: P<Node>,
) -> bool {
    !copies_function_own_bindings(ctx, callback)
        && !callback_shadows_constructor_root(ctx, callback, constructor_callee)
        && !callback_shadows_names_used_by_thunk_parameters(ctx, thunk, callback)
        && !thunk_parameters_captured_or_shadowed(ctx, thunk, callback, receiver)
}

// go: internal/fixables/promise_all_in_effect.go:174 copiesFunctionOwnBindings 69065532651bc318
fn copies_function_own_bindings(ctx: &mut Context<'_, '_>, callback: P<Node>) -> bool {
    let mut copied: Vec<Option<P<Node>>> =
        callback.parameters().iter().copied().map(Some).collect();
    copied.push(callback.type_node());
    copied.push(callback.body());
    for part in copied {
        if part.is_some() && ctx.type_parser.uses_function_own_bindings(callback, part) {
            return true;
        }
    }
    false
}

// go: internal/fixables/promise_all_in_effect.go:184 shadowedOrUnresolvedInCallback d8d0e75be22c863b
fn shadowed_or_unresolved_in_callback(
    ctx: &mut Context<'_, '_>,
    callback: P<Node>,
    name: &str,
    symbol: Option<P<Symbol>>,
) -> bool {
    symbol.is_none()
        || ctx.checker().resolve_name_exported(
            name,
            callback,
            SymbolFlags::Value | SymbolFlags::Alias,
            false,
        ) != symbol
}

// go: internal/fixables/promise_all_in_effect.go:188 callbackShadowsConstructorRoot b26d7de1c5463615
fn callback_shadows_constructor_root(
    ctx: &mut Context<'_, '_>,
    callback: P<Node>,
    constructor_callee: P<Node>,
) -> bool {
    let mut root = constructor_callee;
    while ast::is_property_access_expression(root) {
        root = root.as_property_access_expression().expression();
    }
    !ast::is_identifier(root) || {
        let symbol = ctx.type_parser.get_symbol_at_location(root);
        shadowed_or_unresolved_in_callback(ctx, callback, root.text(), symbol)
    }
}

// go: internal/fixables/promise_all_in_effect.go:196 callbackShadowsNamesUsedByThunkParameters dcee2e89cde1037c
//
// Rust: Go ranges over the callback's locals (a map); the result does not depend on the order, since any hit
// returns true.
fn callback_shadows_names_used_by_thunk_parameters(
    ctx: &mut Context<'_, '_>,
    thunk: P<Node>,
    callback: P<Node>,
) -> bool {
    let entries = callback.locals().map_or_default(|locals| locals.entries());
    for (name, local) in entries {
        let outer = ctx.checker().resolve_name_exported(name, thunk, SymbolFlags::All, false);
        let Some(outer) = outer else {
            continue;
        };
        if outer == local {
            continue;
        }
        for &thunk_parameter in thunk.parameters() {
            if !ctx.type_parser.references_to_symbol(thunk_parameter, outer).is_empty() {
                return true;
            }
        }
    }
    false
}

// go: internal/fixables/promise_all_in_effect.go:211 thunkParametersCapturedOrShadowed 2dd1f01b9eb7a692
fn thunk_parameters_captured_or_shadowed(
    ctx: &mut Context<'_, '_>,
    thunk: P<Node>,
    callback: P<Node>,
    receiver: P<Node>,
) -> bool {
    for &thunk_parameter in thunk.parameters() {
        let Some(name) = thunk_parameter.name().filter(|&name| ast::is_identifier(name)) else {
            return true;
        };
        let symbol = ctx.type_parser.get_symbol_at_location(name);
        if !ctx.type_parser.references_to_symbol(receiver, symbol).is_empty()
            || shadowed_or_unresolved_in_callback(ctx, callback, name.text(), symbol)
        {
            return true;
        }
        for &callback_parameter in callback.parameters() {
            if !ctx.type_parser.references_to_symbol(callback_parameter, symbol).is_empty() {
                return true;
            }
        }
    }
    false
}

// go: internal/fixables/promise_all_in_effect.go:231 newPromiseAllItemCallback e2a75663a551805e
//
// Rust: Go's tracker is used only to build nodes, so the helper takes the factory (callers pass the tracker).
fn new_promise_all_item_callback(
    tracker: &NodeFactory,
    thunk: P<Node>,
    callback: P<Node>,
    constructor_callee: P<Node>,
) -> P<Node> {
    let mut modifiers = None;
    if ast::get_combined_modifier_flags(callback).intersects(ModifierFlags::Async) {
        modifiers = Some(tracker.new_modifier_list(vec![tracker.new_modifier(Kind::AsyncKeyword)]));
    }
    let item_thunk = tracker.new_arrow_function(
        modifiers,
        None,
        Some(tracker.new_node_list(clone_nodes(tracker, thunk.parameters()))),
        tracker.deep_clone_node(callback.type_node()),
        None,
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        tracker.deep_clone_node(callback.body()),
    );
    let item_effect = tracker.new_call_expression(
        tracker.deep_clone_node(Some(constructor_callee)).unwrap(),
        None,
        None,
        tracker.new_node_list(vec![item_thunk]),
        NodeFlags::None,
    );
    tracker.new_arrow_function(
        None,
        None,
        Some(tracker.new_node_list(clone_nodes(tracker, callback.parameters()))),
        None,
        None,
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        Some(item_effect),
    )
}

// go: internal/fixables/promise_all_in_effect.go:263 cloneNodes d5c6c515cbb46afd
fn clone_nodes(tracker: &NodeFactory, nodes: &[P<Node>]) -> Vec<P<Node>> {
    let mut clones = Vec::with_capacity(nodes.len());
    for &node in nodes {
        clones.push(tracker.deep_clone_node(Some(node)).unwrap());
    }
    clones
}
