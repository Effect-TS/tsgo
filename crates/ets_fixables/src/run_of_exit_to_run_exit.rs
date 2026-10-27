use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_OF_EFFECT_EXIT_RE_IMPLEMENTS_EFFECT_1_USE_THE_DEDICATED_EXIT_RUNNER_DIRECTLY_EFFECT_RUN_OF_EXIT_TO_RUN_EXIT;

// go: internal/fixables/run_of_exit_to_run_exit.go:11 RunOfExitToRunExitFix
pub static RUN_OF_EXIT_TO_RUN_EXIT_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "runOfExitToRunExit",
    description: "Replace with the dedicated Effect Exit runner",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["runOfExitToRunExit_fix"],
    run: run_run_of_exit_to_run_exit_fix,
});

// go: internal/fixables/run_of_exit_to_run_exit.go:21 runRunOfExitToRunExitFix
//
// Rust: the match's exit transformation is never nil (an owned copy), so only the callee is checked.
fn run_run_of_exit_to_run_exit_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_run_of_exit_to_run_exit(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let Some(runner_callee) = m.runner_callee else {
            continue;
        };

        let description = "Replace with Effect.".to_string() + m.replacement_name;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                if !remove_piping_flow_transformation(
                    tracker,
                    Some(sf),
                    Some(&m.exit_transformation),
                ) {
                    return;
                }
                replace_effect_method_callee(tracker, sf, Some(runner_callee), m.replacement_name);
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
