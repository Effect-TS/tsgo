use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::NodeFlags;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_EFFECT_CALLBACK_REGISTER_IS_EMPTY_SO_THE_EFFECT_NEVER_COMPLETES_USE_EFFECT_NEVER_INSTEAD_EFFECT_EMPTY_CALLBACK_TO_NEVER;

// go: internal/fixables/empty_callback_to_never.go:12 EmptyCallbackToNeverFix 8365d5731d96190a
pub static EMPTY_CALLBACK_TO_NEVER_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "emptyCallbackToNever",
    description: "Replace an empty Effect.callback register with Effect.never",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["emptyCallbackToNever_fix"],
    run: run_empty_callback_to_never_fix,
});

// go: internal/fixables/empty_callback_to_never.go:22 runEmptyCallbackToNeverFix 5240f65336fea760
fn run_empty_callback_to_never_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_empty_callback_to_never(&mut ctx.type_parser, source_file) {
        let Some(effect_module) = m.effect_module else {
            continue;
        };
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.never".to_string(),
            run: Box::new(move |tracker| {
                let replacement = tracker.new_property_access_expression(
                    tracker.deep_clone_node(Some(effect_module)).unwrap(),
                    None,
                    tracker.new_identifier("never"),
                    NodeFlags::None,
                );
                tracker.replace_piping_flow_prefix(
                    Some(m.source_file),
                    Some(&m.flow),
                    1,
                    Some(replacement),
                );
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
