use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_SUCCEED_EXPRESSES_THIS_CONSTANT_VALUE_MORE_DIRECTLY_THAN_EFFECT_SYNC_EFFECT_SYNC_TO_SUCCEED;

// go: internal/fixables/sync_to_succeed.go:12 SyncToSucceedFix
pub static SYNC_TO_SUCCEED_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "syncToSucceed",
    description: "Replace Effect.sync with Effect.succeed",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["syncToSucceed_fix"],
    run: run_sync_to_succeed_fix,
});

// go: internal/fixables/sync_to_succeed.go:22 runSyncToSucceedFix
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
                tracker.replace_node(
                    sf,
                    Some(m.callee),
                    Some(
                        tracker.new_property_access_path(
                            &ets_typeparser::find_effect_module_reference(Some(sf))
                                .append(&["succeed"]),
                        ),
                    ),
                    None,
                );
                tracker.replace_node(sf, Some(m.thunk), Some(m.constant_value), None);
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
