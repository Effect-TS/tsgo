use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_EXPRESSES_THIS_NON_EFFECTFUL_FOLD_MORE_DIRECTLY_THAN_EFFECT_1_WITH_EFFECT_SUCCEED_HANDLERS_EFFECT_MATCH_EFFECT_TO_MATCH;

// go: internal/fixables/match_effect_to_match.go:12 MatchEffectToMatchFix 6acfd3f3fedb6961
pub static MATCH_EFFECT_TO_MATCH_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "matchEffectToMatch",
    description: "Replace Effect.matchEffect with Effect.match and unwrap Effect.succeed handlers",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["matchEffectToMatch_fix"],
    run: run_match_effect_to_match_fix,
});

// go: internal/fixables/match_effect_to_match.go:18 runMatchEffectToMatchFix be8ebc28afe6f50e
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
                tracker.replace_node(
                    m.source_file,
                    Some(m.callee_node),
                    Some(
                        tracker.new_property_access_path(
                            &ets_typeparser::find_effect_module_reference(Some(m.source_file))
                                .append(&[m.replacement_name]),
                        ),
                    ),
                    None,
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
