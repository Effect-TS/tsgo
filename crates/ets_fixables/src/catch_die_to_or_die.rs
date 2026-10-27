use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_OR_DIE_EXPRESSES_ESCALATING_EVERY_TYPED_FAILURE_INTO_A_DEFECT_MORE_DIRECTLY_THAN_EFFECT_0_WITH_AN_IDENTITY_FORWARDING_EFFECT_DIE_HANDLER_EFFECT_CATCH_DIE_TO_OR_DIE;

// go: internal/fixables/catch_die_to_or_die.go:12 CatchDieToOrDieFix 3832b690afdb1d04
pub static CATCH_DIE_TO_OR_DIE_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "catchDieToOrDie",
    description: "Replace the catch-all Effect.die handler with Effect.orDie",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["catchDieToOrDie_fix"],
    run: run_catch_die_to_or_die_fix,
});

// go: internal/fixables/catch_die_to_or_die.go:22 runCatchDieToOrDieFix f4771d57ebe9b9fb
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
                let or_die = tracker.new_property_access_path(
                    &ets_typeparser::find_effect_module_reference(Some(source_file))
                        .append(&["orDie"]),
                );
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
