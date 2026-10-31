use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THE_PROMISE_RETURNED_BY_0_IS_DISCARDED_SO_ANY_FAILURE_OR_DEFECT_BECOMES_AN_UNHANDLED_REJECTION_USE_1_TO_RUN_IT_FIRE_AND_FORGET_AS_A_FIBER_EFFECT_DISCARDED_RUN_PROMISE_TO_RUN_FORK;

// go: internal/fixables/discarded_run_promise_to_run_fork.go:11 DiscardedRunPromiseToRunForkFix 5c30d9ddac2b46ca
pub static DISCARDED_RUN_PROMISE_TO_RUN_FORK_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "discardedRunPromiseToRunFork",
    description: "Replace with the fire-and-forget Fiber runner",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["discardedRunPromiseToRunFork_fix"],
    run: run_discarded_run_promise_to_run_fork_fix,
});

// go: internal/fixables/discarded_run_promise_to_run_fork.go:21 runDiscardedRunPromiseToRunForkFix f0d8110126996a16
fn run_discarded_run_promise_to_run_fork_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_discarded_run_promise_to_run_fork(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let Some(rename_node) = m.rename_node else {
            continue;
        };

        // The result is discarded, so renaming the runner in place keeps the
        // receiver, the arguments and any surrounding void untouched.
        // ReplacePipingFlowTransformation would rebuild the call and reprint
        // its arguments, losing their comments, and could not rebuild a
        // ManagedRuntime receiver or a curried factory call that flows keep
        // as the step's callee.
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with ".to_string() + m.replacement_name,
            run: Box::new(move |tracker| {
                tracker.replace_node(
                    sf,
                    Some(rename_node),
                    Some(tracker.new_identifier(m.rename_to)),
                    None,
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
