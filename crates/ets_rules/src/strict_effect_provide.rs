use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

use crate::run_effect_inside_effect::{PLATFORM_RUN_MAIN_API, match_run_effect_api};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_PROVIDE_WITH_A_LAYER_SHOULD_ONLY_BE_USED_AT_APPLICATION_ENTRY_POINTS_IF_THIS_IS_AN_ENTRY_POINT_YOU_CAN_SAFELY_DISABLE_THIS_DIAGNOSTIC_OTHERWISE_USING_EFFECT_PROVIDE_MAY_BREAK_SCOPE_LIFETIMES_COMPOSE_ALL_LAYERS_AT_YOUR_ENTRY_POINT_AND_PROVIDE_THEM_AT_ONCE_EFFECT_STRICT_EFFECT_PROVIDE;

// go: internal/rules/strict_effect_provide.go:13 StrictEffectProvide eaeaf6be4f6eb619
// StrictEffectProvide warns when Effect.provide is called with a Layer argument,
// unless the provided Effect goes straight into a runner. This rule is disabled by default.
pub static STRICT_EFFECT_PROVIDE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "strictEffectProvide",
    group: "antipattern",
    description: "Warns when using Effect.provide with layers outside of application entry points",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        // Stack-based traversal
        let mut node_to_visit: Vec<P<Node>> = Vec::new();
        ctx.source_file.as_node().for_each_child(&mut |child| {
            node_to_visit.push(child);
            false
        });

        while let Some(node) = node_to_visit.pop() {
            if node.kind() == Kind::CallExpression
                && let Some(diag) = check_effect_provide_with_layer(ctx, node)
            {
                diags.push(diag);
            }

            // Enqueue children
            node.for_each_child(&mut |child| {
                node_to_visit.push(child);
                false
            });
        }

        diags
    },
});

// go: internal/rules/strict_effect_provide.go:50 checkEffectProvideWithLayer c5b53d5aaa532be8
// checkEffectProvideWithLayer checks if a call expression is Effect.provide(...) with a Layer argument.
pub(crate) fn check_effect_provide_with_layer(
    ctx: &mut Context<'_, '_>,
    node: P<Node>,
) -> Option<P<Diagnostic>> {
    if node.kind() != Kind::CallExpression {
        return None;
    }
    let call = node.as_call_expression();
    let args = call.arguments().nodes();
    if args.is_empty() {
        return None;
    }

    // Check if the expression references Effect.provide
    if !ctx.type_parser.is_node_reference_to_effect_module_api(call.expression(), "provide") {
        return None;
    }

    // Check if any argument is a Layer type
    for &arg in args {
        if ctx.type_parser.node_cannot_be_effect_library_type(arg) {
            continue;
        }
        let Some(arg_type) = ctx.type_parser.get_type_at_location(arg) else {
            continue;
        };
        if ctx.type_parser.layer_type(arg_type).is_some() {
            // Providing layers right before running the Effect is what an entry point does
            if is_effect_provide_passed_to_runner(ctx.type_parser, node) {
                return None;
            }
            // Found a Layer argument — emit diagnostic on the call expression
            return Some(ctx.new_diagnostic(
                ctx.source_file,
                ctx.get_error_range(node),
                MESSAGE,
                &[],
                &[],
            ));
        }
    }

    None
}

// go: internal/rules/strict_effect_provide.go:90 isEffectProvidePassedToRunner 479ac659129dbde5
// isEffectProvidePassedToRunner reports whether the Effect returned by a provide call goes straight
// into a runner: the provide call, or the pipe ending with it, is the runner's argument, or the
// runner is the next step of the same pipe.
fn is_effect_provide_passed_to_runner(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    let mut expr = skip_outer_parentheses(node);
    // A data-last provide applied to its Effect, as in Effect.provide(layer)(program)
    if let Some(parent) = expr.parent()
        && parent.kind() == Kind::CallExpression
        && parent.as_call_expression().expression() == expr
    {
        expr = skip_outer_parentheses(parent);
    }
    if let Some(parent) = expr.parent()
        && parent.kind() == Kind::CallExpression
        && let Some(pipe) = tp.parse_pipe_call(parent)
    {
        let args = pipe.args();
        for i in 0..args.len() {
            if *args.at(i) != expr {
                continue;
            }
            if i + 1 < args.len() {
                return is_effect_runner_pipe_step(tp, *args.at(i + 1));
            }
            expr = skip_outer_parentheses(pipe.node());
            break;
        }
    }

    let Some(parent) = expr.parent() else {
        return false;
    };
    if parent.kind() != Kind::CallExpression {
        return false;
    }
    let call = parent.as_call_expression();
    if call.arguments().nodes().first() != Some(&expr) {
        return false;
    }
    is_effect_runner(tp, call.expression())
}

// go: internal/rules/strict_effect_provide.go:125 isEffectRunnerPipeStep 718dfe5cee8fcbd3
// isEffectRunnerPipeStep reports whether a pipe step runs the Effect: a runner, or a platform
// runMain applied to its options.
fn is_effect_runner_pipe_step(tp: &mut TypeParser<'_>, step: P<Node>) -> bool {
    let step = ast::skip_parentheses(step);
    if is_effect_runner(tp, step) {
        return true;
    }
    step.kind() == Kind::CallExpression
        && tp.is_node_reference_to_platform_runtime_api(
            step.as_call_expression().expression(),
            PLATFORM_RUN_MAIN_API,
        )
}

// go: internal/rules/strict_effect_provide.go:135 isEffectRunner fd95db51aee99e51
// isEffectRunner reports whether node references one of the Effect.run* APIs or a platform runMain.
fn is_effect_runner(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    !match_run_effect_api(tp, node).is_empty()
        || tp.is_node_reference_to_platform_runtime_api(node, PLATFORM_RUN_MAIN_API)
}

// go: internal/rules/strict_effect_provide.go:140 skipOuterParentheses d0c3967cffb08b5d
// skipOuterParentheses returns the outermost parenthesized expression wrapping node, or node itself.
fn skip_outer_parentheses(mut node: P<Node>) -> P<Node> {
    while let Some(parent) = node.parent()
        && parent.kind() == Kind::ParenthesizedExpression
    {
        node = parent;
    }
    node
}
