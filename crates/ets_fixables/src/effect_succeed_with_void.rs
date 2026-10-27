use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_VOID_REPRESENTS_THE_SAME_OUTCOME_AS_EFFECT_SUCCEED_UNDEFINED_OR_EFFECT_SUCCEED_VOID_0_EFFECT_EFFECT_SUCCEED_WITH_VOID;

// go: internal/fixables/effect_succeed_with_void.go:13 EffectSucceedWithVoidFix 7377d505f5f97ba8
pub static EFFECT_SUCCEED_WITH_VOID_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "effectSucceedWithVoid",
    description: "Replace with Effect.void",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["effectSucceedWithVoid_fix"],
    run: run_effect_succeed_with_void_fix,
});

// go: internal/fixables/effect_succeed_with_void.go:21 runEffectSucceedWithVoidFix ccc09e1c9b686498
fn run_effect_succeed_with_void_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_effect_succeed_with_void(&mut ctx.type_parser, sf);
    for m in matches {
        let diag_range = m.location;
        if !diag_range.intersects(ctx.span) && !ctx.span.contained_by(diag_range) {
            continue;
        }
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.void".to_string(),
            run: Box::new(move |tracker| {
                let replacement_node = tracker.new_property_access_path(
                    &ets_typeparser::find_effect_module_reference(Some(sf)).append(&["void"]),
                );
                ast::set_parent_in_children(replacement_node);
                tracker.replace_node(sf, Some(m.call_node), Some(replacement_node), None);
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
