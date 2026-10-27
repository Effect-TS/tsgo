use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rules::YieldNowForm;
use ets_shim::ast::NodeFlags;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::CONSIDER_EFFECT_YIELD_NOW_FOR_COOPERATIVE_YIELDING_INSTEAD_OF_SLEEPING_FOR_ZERO_DURATION_EFFECT_EFFECT_SLEEP_ZERO_TO_YIELD_NOW;

// go: internal/fixables/effect_sleep_zero_to_yield_now.go:13 EffectSleepZeroToYieldNowFix
pub static EFFECT_SLEEP_ZERO_TO_YIELD_NOW_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "effectSleepZeroToYieldNow",
    description: "Replace zero-duration sleep with cooperative yielding",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["effectSleepZeroToYieldNow_fix"],
    run: run_effect_sleep_zero_to_yield_now_fix,
});

// go: internal/fixables/effect_sleep_zero_to_yield_now.go:23 runEffectSleepZeroToYieldNowFix
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
                let mut replacement = tracker.new_property_access_path(
                    &ets_typeparser::find_effect_module_reference(Some(m.source_file))
                        .append(&["yieldNow"]),
                );
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
