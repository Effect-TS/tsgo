use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_MAP_EXPRESSES_THIS_SUCCESS_VALUE_TRANSFORMATION_MORE_DIRECTLY_THAN_EFFECT_FLAT_MAP_FOLLOWED_BY_EFFECT_SUCCEED_EFFECT_FLAT_MAP_TO_MAP;

// go: internal/fixables/flat_map_to_map.go:12 FlatMapToMapFix fed59567fbdf298c
pub static FLAT_MAP_TO_MAP_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "flatMapToMap",
    description: "Replace Effect.flatMap + Effect.succeed with Effect.map",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["flatMapToMap_fix"],
    run: run_flat_map_to_map_fix,
});

// go: internal/fixables/flat_map_to_map.go:22 runFlatMapToMapFix 90c5618768b94f0d
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
                tracker.replace_node(
                    sf,
                    Some(m.callee),
                    Some(tracker.new_property_access_path(
                        &ets_typeparser::find_effect_module_reference(Some(sf)).append(&["map"]),
                    )),
                    None,
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
