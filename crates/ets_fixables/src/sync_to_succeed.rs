use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::Effect_succeed_expresses_this_constant_value_more_directly_than_Effect_sync_effect_syncToSucceed;

// go: internal/fixables/sync_to_succeed.go:11 SyncToSucceedFix
pub static SYNC_TO_SUCCEED_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "syncToSucceed",
    description: "Replace Effect.sync with Effect.succeed",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["syncToSucceed_fix"],
    run: run_sync_to_succeed_fix,
});

// go: internal/fixables/sync_to_succeed.go:21 runSyncToSucceedFix
//
// Rust: the match's callee is never nil, so Go's nil check drops.
fn run_sync_to_succeed_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_sync_to_succeed(&mut ctx.type_parser, sf) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.succeed".to_string(),
            run: Box::new(move |tracker| {
                replace_effect_method_callee(tracker, sf, Some(m.callee), m.callee_name, "succeed");
                tracker.replace_node(sf, Some(m.thunk), Some(m.constant_value), None);
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
