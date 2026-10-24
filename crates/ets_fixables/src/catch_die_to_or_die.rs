use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::Effect_orDie_expresses_escalating_every_typed_failure_into_a_defect_more_directly_than_Effect_0_with_an_identity_forwarding_Effect_die_handler_effect_catchDieToOrDie;

// go: internal/fixables/catch_die_to_or_die.go:11 CatchDieToOrDieFix
pub static CATCH_DIE_TO_OR_DIE_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "catchDieToOrDie",
    description: "Replace the catch-all Effect.die handler with Effect.orDie",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["catchDieToOrDie_fix"],
    run: run_catch_die_to_or_die_fix,
});

// go: internal/fixables/catch_die_to_or_die.go:21 runCatchDieToOrDieFix
//
// Rust: the match's transformation is never nil (`CatchDieToOrDieMatch::transformation` is not optional), so only
// Go's `HasTypeArguments` check remains.
fn run_catch_die_to_or_die_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_catch_die_to_or_die(&mut ctx.type_parser, Some(source_file)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if m.has_type_arguments {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.orDie".to_string(),
            run: Box::new(move |tracker| {
                let or_die = effect_module_method(tracker, source_file, m.effect_module, "orDie");
                tracker.replace_piping_flow_transformation(
                    Some(source_file),
                    Some(&m.transformation),
                    PipingFlowTransformationReplacement {
                        callee: Some(or_die),
                        type_arguments: None,
                        arguments: None,
                    },
                );
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
