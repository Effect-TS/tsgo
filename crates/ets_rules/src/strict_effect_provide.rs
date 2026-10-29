use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{PipingFlowTransformation, TypeParser};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_PROVIDE_WITH_A_LAYER_SHOULD_ONLY_BE_USED_AT_APPLICATION_ENTRY_POINTS_IF_THIS_IS_AN_ENTRY_POINT_YOU_CAN_SAFELY_DISABLE_THIS_DIAGNOSTIC_OTHERWISE_USING_EFFECT_PROVIDE_MAY_BREAK_SCOPE_LIFETIMES_COMPOSE_ALL_LAYERS_AT_YOUR_ENTRY_POINT_AND_PROVIDE_THEM_AT_ONCE_EFFECT_STRICT_EFFECT_PROVIDE;

// go: internal/rules/strict_effect_provide.go:13 StrictEffectProvide 171f983b7aec0b9c
// StrictEffectProvide warns when Effect.provide is called with a Layer argument,
// unless the next step of the flow runs the Effect. This rule is disabled by default.
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
        let flows = ctx.type_parser.piping_flows(ctx.source_file, true);
        for flow in flows.as_deref().unwrap_or_default() {
            let transformations = flow.transformations();
            for (index, transformation) in transformations.all() {
                if !is_effect_provide_with_layer(ctx, transformation) {
                    continue;
                }
                // Providing layers right before running the Effect is what an entry point does
                if index + 1 < transformations.len()
                    && is_entry_point_runner(
                        ctx.type_parser,
                        transformations.at(index + 1).callee(),
                    )
                {
                    continue;
                }
                if let Some(node) = transformation_application_node(Some(transformation)) {
                    diags.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ctx.get_error_range(node),
                        MESSAGE,
                        &[],
                        &[],
                    ));
                }
            }
        }
        diags
    },
});

// go: internal/rules/strict_effect_provide.go:42 isEffectProvideWithLayer 4006f4af616c37d5
// isEffectProvideWithLayer checks if a transformation is Effect.provide(...) with a Layer argument.
pub(crate) fn is_effect_provide_with_layer(
    ctx: &mut Context<'_, '_>,
    transformation: &PipingFlowTransformation,
) -> bool {
    if transformation.args().len() == 0
        || !ctx
            .type_parser
            .is_node_reference_to_effect_module_api(transformation.callee(), "provide")
    {
        return false;
    }
    for (_, &arg) in transformation.args().all() {
        if ctx.type_parser.layer_type_at_location(arg).is_some() {
            return true;
        }
    }
    false
}

// go: internal/rules/strict_effect_provide.go:55 entryPointRunnerApis 2eefb458cdf3e6e7
// entryPointRunnerApis lists the Effect module APIs that run an Effect at an entry point.
static ENTRY_POINT_RUNNER_APIS: &[&str] =
    &["runPromise", "runSync", "runFork", "runPromiseExit", "runSyncExit", "runCallback"];

// go: internal/rules/strict_effect_provide.go:58 isEntryPointRunner 52109102679aca48
// isEntryPointRunner reports whether node references an Effect.run* API or a platform runMain.
fn is_entry_point_runner(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    for name in ENTRY_POINT_RUNNER_APIS {
        if tp.is_node_reference_to_effect_module_api(node, name) {
            return true;
        }
    }
    tp.is_node_reference_to_platform_runtime_api(node, "runMain")
}
