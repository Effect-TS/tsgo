use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::Effect_0_expresses_this_non_effectful_fold_more_directly_than_Effect_1_with_Effect_succeed_handlers_effect_matchEffectToMatch;

// go: internal/fixables/match_effect_to_match.go:11 MatchEffectToMatchFix
pub static MATCH_EFFECT_TO_MATCH_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "matchEffectToMatch",
    description: "Replace Effect.matchEffect with Effect.match and unwrap Effect.succeed handlers",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["matchEffectToMatch_fix"],
    run: run_match_effect_to_match_fix,
});

// go: internal/fixables/match_effect_to_match.go:17 runMatchEffectToMatchFix
//
// Rust: the match's callee node is never nil (a non-optional field), so Go's nil check drops.
fn run_match_effect_to_match_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_match_effect_to_match(&mut ctx.type_parser, Some(source_file)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: format!("Replace with Effect.{}", m.replacement_name),
            run: Box::new(move |tracker| {
                replace_effect_method_callee(
                    tracker,
                    m.source_file,
                    Some(m.callee_node),
                    m.callee_name_node,
                    m.replacement_name,
                );
                for i in 0..m.handler_results.len() {
                    tracker.replace_node(
                        m.source_file,
                        Some(m.handler_results[i]),
                        Some(m.succeed_arguments[i]),
                        None,
                    );
                }
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
