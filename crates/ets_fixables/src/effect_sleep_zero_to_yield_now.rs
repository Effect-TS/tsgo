use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rules::YieldNowForm;
use ets_shim::ast::NodeFlags;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::Consider_Effect_yieldNow_for_cooperative_yielding_instead_of_sleeping_for_zero_duration_effect_effectSleepZeroToYieldNow;

// go: internal/fixables/effect_sleep_zero_to_yield_now.go:12 EffectSleepZeroToYieldNowFix
pub static EFFECT_SLEEP_ZERO_TO_YIELD_NOW_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "effectSleepZeroToYieldNow",
    description: "Replace zero-duration sleep with cooperative yielding",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["effectSleepZeroToYieldNow_fix"],
    run: run_effect_sleep_zero_to_yield_now_fix,
});

// go: internal/fixables/effect_sleep_zero_to_yield_now.go:22 runEffectSleepZeroToYieldNowFix
fn run_effect_sleep_zero_to_yield_now_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_effect_sleep_zero_to_yield_now(&mut ctx.type_parser, source_file) {
        if m.effect_module.is_none()
            || (!m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location))
        {
            continue;
        }
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.yieldNow".to_string(),
            run: Box::new(move |tracker| {
                let mut replacement =
                    effect_module_method(tracker, m.source_file, m.effect_module, "yieldNow");
                if m.form == YieldNowForm::Call {
                    replacement = tracker.new_call_expression(
                        replacement,
                        None,
                        None,
                        tracker.new_node_list(Vec::new()),
                        NodeFlags::None,
                    );
                }
                tracker.replace_node(m.source_file, Some(m.call), Some(replacement), None);
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
