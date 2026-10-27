use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_MAP_BOTH_EXPRESSES_THESE_FAILURE_AND_SUCCESS_TRANSFORMATIONS_MORE_DIRECTLY_THAN_EFFECT_MATCH_EFFECT_EFFECT_MATCH_EFFECT_TO_MAP_BOTH;

// go: internal/fixables/match_effect_to_map_both.go:12 MatchEffectToMapBothFix
pub static MATCH_EFFECT_TO_MAP_BOTH_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "matchEffectToMapBoth",
    description: "Replace Effect.matchEffect with Effect.mapBoth and unwrap Effect.fail and Effect.succeed handlers",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["matchEffectToMapBoth_fix"],
    run: run_match_effect_to_map_both_fix,
});

// go: internal/fixables/match_effect_to_map_both.go:18 runMatchEffectToMapBothFix
//
// Rust: the match's callee node is never nil (a non-optional field), so Go's nil check drops.
fn run_match_effect_to_map_both_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_match_effect_to_map_both(&mut ctx.type_parser, Some(source_file)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.mapBoth".to_string(),
            run: Box::new(move |tracker| {
                tracker.replace_node(
                    m.source_file,
                    Some(m.callee_node),
                    Some(
                        tracker.new_property_access_path(
                            &ets_typeparser::find_effect_module_reference(Some(m.source_file))
                                .append(&["mapBoth"]),
                        ),
                    ),
                    None,
                );
                for i in 0..m.handler_results.len() {
                    tracker.replace_node(
                        m.source_file,
                        Some(m.handler_results[i]),
                        Some(m.constructor_args[i]),
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
