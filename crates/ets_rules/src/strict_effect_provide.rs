use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_PROVIDE_WITH_A_LAYER_SHOULD_ONLY_BE_USED_AT_APPLICATION_ENTRY_POINTS_IF_THIS_IS_AN_ENTRY_POINT_YOU_CAN_SAFELY_DISABLE_THIS_DIAGNOSTIC_OTHERWISE_USING_EFFECT_PROVIDE_MAY_BREAK_SCOPE_LIFETIMES_COMPOSE_ALL_LAYERS_AT_YOUR_ENTRY_POINT_AND_PROVIDE_THEM_AT_ONCE_EFFECT_STRICT_EFFECT_PROVIDE;

// go: internal/rules/strict_effect_provide.go:12 StrictEffectProvide
// StrictEffectProvide warns when Effect.provide is called with a Layer argument.
// This rule is disabled by default.
pub static STRICT_EFFECT_PROVIDE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "strictEffectProvide",
    group: "antipattern",
    description: "Warns when using Effect.provide with layers outside of application entry points",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
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

// go: internal/rules/strict_effect_provide.go:49 checkEffectProvideWithLayer
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
