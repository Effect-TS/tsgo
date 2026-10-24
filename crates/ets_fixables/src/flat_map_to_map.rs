use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::Effect_map_expresses_this_success_value_transformation_more_directly_than_Effect_flatMap_followed_by_Effect_succeed_effect_flatMapToMap;

// go: internal/fixables/flat_map_to_map.go:11 FlatMapToMapFix
pub static FLAT_MAP_TO_MAP_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "flatMapToMap",
    description: "Replace Effect.flatMap + Effect.succeed with Effect.map",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["flatMapToMap_fix"],
    run: run_flat_map_to_map_fix,
});

// go: internal/fixables/flat_map_to_map.go:21 runFlatMapToMapFix
fn run_flat_map_to_map_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_flat_map_to_map(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.map".to_string(),
            run: Box::new(move |tracker| {
                replace_effect_method_callee(
                    tracker,
                    sf,
                    Some(m.callee),
                    m.callee_name_node,
                    "map",
                );
                tracker.replace_node(
                    sf,
                    Some(m.succeed_call_expression),
                    Some(m.succeed_argument),
                    None,
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
