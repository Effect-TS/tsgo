use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_OR_ELSE_SUCCEED_EXPRESSES_THE_SAME_RECOVERY_MORE_DIRECTLY_THAN_EFFECT_0_FOLLOWED_BY_EFFECT_SUCCEED_EFFECT_CATCH_TO_OR_ELSE_SUCCEED;

// go: internal/fixables/catch_to_or_else_succeed.go:11 CatchToOrElseSucceedFix
pub static CATCH_TO_OR_ELSE_SUCCEED_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "catchToOrElseSucceed",
    description: "Replace Effect.catch + Effect.succeed with Effect.orElseSucceed",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["catchToOrElseSucceed_fix"],
    run: run_catch_to_or_else_succeed_fix,
});

// go: internal/fixables/catch_to_or_else_succeed.go:19 runCatchToOrElseSucceedFix
fn run_catch_to_or_else_succeed_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_catch_to_or_else_succeed(&mut ctx.type_parser, sf);
    for m in matches {
        let diag_range = m.location;
        if !diag_range.intersects(ctx.span) && !ctx.span.contained_by(diag_range) {
            continue;
        }
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.orElseSucceed".to_string(),
            run: Box::new(move |tracker| {
                replace_effect_method_callee(tracker, sf, Some(m.callee), "orElseSucceed");

                tracker.replace_node(
                    sf,
                    Some(m.succeed_call_expression),
                    Some(m.succeed_argument),
                    None,
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
