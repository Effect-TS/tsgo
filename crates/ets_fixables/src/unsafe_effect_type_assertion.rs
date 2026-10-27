use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_TYPE_ASSERTION_UNSAFELY_NARROWS_THE_ERROR_OR_REQUIREMENTS_CHANNELS_EFFECT_UNSAFE_EFFECT_TYPE_ASSERTION;

// go: internal/fixables/unsafe_effect_type_assertion.go:11 UnsafeEffectTypeAssertionFix d399f9b99290f2cd
pub static UNSAFE_EFFECT_TYPE_ASSERTION_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "unsafeEffectTypeAssertion",
    description: "Remove the unsafe assertion",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["unsafeEffectTypeAssertion_fix"],
    run: run_unsafe_effect_type_assertion_fix,
});

// go: internal/fixables/unsafe_effect_type_assertion.go:19 runUnsafeEffectTypeAssertionFix 4619fbcf76221179
fn run_unsafe_effect_type_assertion_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    let matches = ets_rules::analyze_unsafe_effect_type_assertion(&mut ctx.type_parser, Some(sf));
    for m in matches {
        let diag_range = m.location;
        if !diag_range.intersects(ctx.span) && !ctx.span.contained_by(diag_range) {
            continue;
        }

        let assertion_node = m.assertion_node;
        let expression_node = m.expression_node;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Remove the unsafe assertion".to_string(),
            run: Box::new(move |tracker| {
                tracker.replace_node(sf, Some(assertion_node), Some(expression_node), None);
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
