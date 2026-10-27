use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, NodeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message =
    &tsdiag::THIS_EFFECT_GEN_CONTAINS_A_SINGLE_RETURN_STATEMENT_EFFECT_UNNECESSARY_EFFECT_GEN;

// go: internal/fixables/unnecessary_effect_gen.go:13 UnnecessaryEffectGenFix
pub static UNNECESSARY_EFFECT_GEN_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "unnecessaryEffectGen",
    description: "Remove the Effect.gen, and keep the body",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["unnecessaryEffectGen_fix"],
    run: run_unnecessary_effect_gen_fix,
});

// go: internal/fixables/unnecessary_effect_gen.go:21 runUnnecessaryEffectGenFix
fn run_unnecessary_effect_gen_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_unnecessary_effect_gen(&mut ctx.type_parser, sf);
    for m in matches {
        let diag_range = m.location;
        if !diag_range.intersects(ctx.span) && !ctx.span.contained_by(diag_range) {
            continue;
        }

        let call_node = m.call_node;
        let yielded_expression = m.yielded_expression;
        if m.explicit_return || m.success_is_void {
            // Simple unwrap: delete the prefix and suffix around the yielded expression
            if let Some(action) = ctx.new_fix_action(FixAction {
                description: "Remove the Effect.gen, and keep the body".to_string(),
                run: Box::new(move |tracker| {
                    tracker.delete_range(
                        sf,
                        TextRange::from_text_pos(call_node.pos(), yielded_expression.pos()),
                    );
                    tracker.delete_range(
                        sf,
                        TextRange::from_text_pos(yielded_expression.end(), call_node.end()),
                    );
                }),
            }) {
                return vec![action];
            }
            continue;
        }

        // No return + non-void success: wrap with Effect.asVoid(...)
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Remove the Effect.gen, and keep the body".to_string(),
            run: Box::new(move |tracker| {
                let as_void_access = effect_module_method(tracker, sf, "asVoid");

                // Clone the yielded expression
                let cloned_expr = tracker.deep_clone_node(Some(yielded_expression)).unwrap();

                // Build Effect.asVoid(expr) call expression
                let call_expr = tracker.new_call_expression(
                    as_void_access,
                    None,
                    None,
                    tracker.new_node_list(vec![cloned_expr]),
                    NodeFlags::None,
                );

                ast::set_parent_in_children(call_expr);
                tracker.replace_node(sf, Some(call_node), Some(call_expr), None);
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
