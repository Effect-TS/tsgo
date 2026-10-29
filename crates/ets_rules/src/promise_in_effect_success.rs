use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node};
use ets_shim::checker::{SignatureKind, Type};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    PipingFlowTransformation, TransformationKind, TypeParser, type_equivalent_node,
};
use rustc_hash::FxHashMap;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::THE_EFFECT_SUCCESS_CHANNEL_CONTAINS_A_PROMISE_THAT_IS_NOT_AWAITED_USE_EFFECT_PROMISE_OR_EFFECT_TRY_PROMISE_TO_REPRESENT_ASYNC_WORK_EFFECT_PROMISE_IN_EFFECT_SUCCESS;

// go: internal/rules/promise_in_effect_success.go:12 PromiseInEffectSuccess f08cc7d8da68a7c4
pub static PROMISE_IN_EFFECT_SUCCESS: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "promiseInEffectSuccess",
    group: "correctness",
    description: "Detects Promise types in Effect success channels where they are not awaited",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();
        struct StackEntry {
            node: P<Node>,
            visited: bool,
        }

        let mut stack = vec![StackEntry { node: ctx.source_file.as_node(), visited: false }];
        let mut matched: FxHashMap<P<Node>, bool> = FxHashMap::default();

        while let Some(entry) = stack.pop() {
            let node = entry.node;

            if !entry.visited {
                if ast::is_type_node(node) {
                    continue;
                }
                stack.push(StackEntry { node, visited: true });
                node.for_each_child(&mut |child| {
                    stack.push(StackEntry { node: child, visited: false });
                    false
                });
                continue;
            }

            if matched.get(&node).copied().unwrap_or(false) {
                if let Some(parent) = node.parent()
                    && !ast::is_function_like(node)
                {
                    matched.insert(parent, true);
                }
                continue;
            }

            if !ast::is_expression(node) || ast::is_declaration_name(node) {
                continue;
            }
            if type_equivalent_node(node).1 {
                continue;
            }

            // Declared-type prefilter: a diagnostic requires a strict Effect
            // flow type on the node, which reference nodes with a
            // conclusively non-Effect declared type — and calls whose
            // resolved signature conclusively cannot return one — can never
            // have. Skipped nodes can never match, so no matched-map
            // bookkeeping is needed.
            if !ctx.type_parser.node_could_be_strict_effect(node) {
                continue;
            }

            let mut t: Option<P<Type>> = None;
            if node.kind() == Kind::CallExpression {
                let signature = ctx.checker().get_resolved_signature_exported(node);
                t = Some(ctx.checker().get_return_type_of_signature(signature));
            }
            if t.is_none() {
                t = ctx.type_parser.get_type_at_location(node);
            }
            let effect = ctx.type_parser.strict_effect_type(t);
            let Some(effect) = effect else {
                continue;
            };
            if !type_contains_promise(ctx.type_parser, Some(effect.a())) {
                continue;
            }

            if has_explicit_promise_effect_context(ctx.type_parser, node)
                || has_explicit_promise_success_type_arguments(ctx.type_parser, node)
                || is_effect_sync_call(ctx.type_parser, Some(node))
            {
                continue;
            }

            diags.push(ctx.new_diagnostic(
                ctx.source_file,
                ctx.get_error_range(node),
                MESSAGE,
                &[],
                &[],
            ));
            if let Some(parent) = node.parent() {
                matched.insert(parent, true);
            }
        }

        diags
    },
});

// go: internal/rules/promise_in_effect_success.go:111 hasExplicitPromiseEffectContext 3816de2a9530cc76
//
// Rust: the checker is the parser's.
pub(crate) fn has_explicit_promise_effect_context(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    let mut current = Some(node);
    while let Some(c) = current {
        match c.kind() {
            Kind::VariableDeclaration | Kind::PropertyDeclaration | Kind::Parameter => {
                return is_explicit_promise_effect_type(tp, c.type_node());
            }
            Kind::AsExpression | Kind::SatisfiesExpression => {
                if is_explicit_promise_effect_type(tp, c.type_node()) {
                    return true;
                }
            }
            Kind::ArrowFunction
            | Kind::FunctionExpression
            | Kind::FunctionDeclaration
            | Kind::MethodDeclaration => {
                if c.kind() == Kind::ArrowFunction
                    && c.as_arrow_function().body().is_some_and(|body| body.kind() != Kind::Block)
                {
                    return has_explicit_promise_effect_return(tp, c);
                }
                return false;
            }
            Kind::ReturnStatement => {
                let function = ast::get_containing_function(c);
                return function
                    .is_some_and(|function| has_explicit_promise_effect_return(tp, function));
            }
            _ => {}
        }
        current = c.parent();
    }
    false
}

// go: internal/rules/promise_in_effect_success.go:133 isExplicitPromiseEffectType 28990f96dbea0633
pub(crate) fn is_explicit_promise_effect_type(
    tp: &mut TypeParser<'_>,
    type_node: Option<P<Node>>,
) -> bool {
    let effect = tp.strict_effect_type_at_location(type_node);
    effect.is_some_and(|effect| type_contains_promise(tp, Some(effect.a())))
}

// go: internal/rules/promise_in_effect_success.go:138 hasExplicitPromiseEffectReturn 1a2183f2ad0ee1bb
//
// Rust: the checker is the parser's.
pub(crate) fn has_explicit_promise_effect_return(
    tp: &mut TypeParser<'_>,
    function: P<Node>,
) -> bool {
    if is_explicit_promise_effect_type(tp, function.type_node()) {
        return true;
    }
    let mut parent = function.parent();
    while let Some(p) = parent
        && p.kind() == Kind::ParenthesizedExpression
    {
        parent = p.parent();
    }
    let Some(parent) = parent else {
        return false;
    };
    let Some(type_node) = parent.type_node() else {
        return false;
    };
    let t = tp.get_type_at_location(type_node);
    for member in tp.unroll_union_members(t).unwrap_or_default() {
        let signatures = tp.checker().get_signatures_of_type(member, SignatureKind::Call);
        for &signature in signatures {
            let return_type = tp.checker().get_return_type_of_signature(signature);
            let result = tp.strict_effect_type(return_type);
            if let Some(result) = result
                && type_contains_promise(tp, Some(result.a()))
            {
                return true;
            }
        }
    }
    false
}

// go: internal/rules/promise_in_effect_success.go:165 typeContainsPromise 2d88b981425a9e66
pub(crate) fn type_contains_promise(tp: &mut TypeParser<'_>, t: Option<P<Type>>) -> bool {
    for member in tp.unroll_union_members(t).unwrap_or_default() {
        if tp.promise_type(member).is_some() {
            return true;
        }
    }
    false
}

// go: internal/rules/promise_in_effect_success.go:176 explicitPromiseSuccessApis b5ecd7089891d36e
// explicitPromiseSuccessApis lists the Effect APIs whose explicit type
// arguments annotate the success channel directly.
pub(crate) static EXPLICIT_PROMISE_SUCCESS_APIS: &[&str] = &["succeed", "as", "map", "zipWith"];

// go: internal/rules/promise_in_effect_success.go:185 hasExplicitPromiseSuccessTypeArguments 8547e54866ab2dd0
// hasExplicitPromiseSuccessTypeArguments reports whether the promise in the
// success channel was written explicitly through type arguments on the node
// itself or on a transformation of the piping flow rooted at the node.
// Flow transformations count at any position when the flow is rooted at a
// pipe call; otherwise only the node's own final transformation counts, so an
// explicit annotation passed as an argument to an unrelated constructor still
// reports.
pub(crate) fn has_explicit_promise_success_type_arguments(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> bool {
    let Some(flow) = tp.longest_piping_flow_at(node, true) else {
        return false;
    };
    if flow.transformations().len() == 0 {
        return false;
    }
    let last = flow.transformations().len() - 1;
    let final_kind = flow.transformations().at(last).kind();
    let pipe_rooted =
        final_kind == TransformationKind::Pipe || final_kind == TransformationKind::Pipeable;
    for (i, _) in flow.transformations().all() {
        if !is_explicit_promise_success_transformation(tp, Some(flow.transformations().at(i))) {
            continue;
        }
        if pipe_rooted || i == last {
            return true;
        }
    }
    false
}

// go: internal/rules/promise_in_effect_success.go:207 isExplicitPromiseSuccessTransformation 81a09532d280412e
// isExplicitPromiseSuccessTransformation reports whether a piping transformation
// applies one of the explicit promise-success APIs with explicit type arguments.
// Callees may be wrapped in parentheses or curried (e.g. Effect.as<...>(value)).
pub(crate) fn is_explicit_promise_success_transformation(
    tp: &mut TypeParser<'_>,
    transformation: Option<&PipingFlowTransformation>,
) -> bool {
    let Some(transformation) = transformation else {
        return false;
    };
    if transformation.type_arguments().is_none_or(|args| args.nodes().is_empty()) {
        return false;
    }
    let mut callee = ast::skip_parentheses(transformation.callee());
    if callee.kind() == Kind::CallExpression {
        callee = callee.as_call_expression().expression();
    }
    is_explicit_promise_success_api(tp, callee)
}

// go: internal/rules/promise_in_effect_success.go:221 isExplicitPromiseSuccessApi 2ac1458b0d2b40f9
pub(crate) fn is_explicit_promise_success_api(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    for name in EXPLICIT_PROMISE_SUCCESS_APIS {
        if tp.is_node_reference_to_effect_module_api(node, name) {
            return true;
        }
    }
    false
}

// go: internal/rules/promise_in_effect_success.go:230 isEffectSyncCall bc7201984377b1f2
pub(crate) fn is_effect_sync_call(tp: &mut TypeParser<'_>, node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if node.kind() != Kind::CallExpression {
        return false;
    }
    tp.is_node_reference_to_effect_module_api(node.as_call_expression().expression(), "sync")
}
