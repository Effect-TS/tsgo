use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rules::YieldNowForm;
use ets_shim::ast::{Kind, Node, NodeFlags};
use ets_shim::core::P;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::TypeParser;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::CONSIDER_EFFECT_YIELD_NOW_FOR_COOPERATIVE_YIELDING_INSTEAD_OF_SLEEPING_FOR_ZERO_DURATION_EFFECT_EFFECT_SLEEP_ZERO_TO_YIELD_NOW;

// go: internal/fixables/effect_sleep_zero_to_yield_now.go:13 EffectSleepZeroToYieldNowFix 5327f75020fe8091
pub static EFFECT_SLEEP_ZERO_TO_YIELD_NOW_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "effectSleepZeroToYieldNow",
    description: "Replace zero-duration sleep with cooperative yielding",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["effectSleepZeroToYieldNow_fix"],
    run: run_effect_sleep_zero_to_yield_now_fix,
});

// go: internal/fixables/effect_sleep_zero_to_yield_now.go:23 runEffectSleepZeroToYieldNowFix c7a3bac63383a807
fn run_effect_sleep_zero_to_yield_now_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_effect_sleep_zero_to_yield_now(&mut ctx.type_parser, source_file) {
        if (!m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location))
            || !effect_sleep_zero_called_on_effect_module(&mut ctx.type_parser, m.call)
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

// go: internal/fixables/effect_sleep_zero_to_yield_now.go:47 effectSleepZeroCalledOnEffectModule fdcf13a10e102965
// effectSleepZeroCalledOnEffectModule reports whether the sleep call is written
// on an imported Effect module, the only spelling the fix rewrites.
fn effect_sleep_zero_called_on_effect_module(tp: &mut TypeParser<'_>, call: P<Node>) -> bool {
    let callee = ets_rules::unwrap_zero_duration_expression(call.as_call_expression().expression());
    if callee.kind() != Kind::PropertyAccessExpression {
        return false;
    }
    let receiver = callee.as_property_access_expression().expression();
    ets_rules::is_imported_module_receiver(tp, receiver)
        && tp.is_expression_effect_module(ets_rules::unwrap_zero_duration_expression(receiver))
}
