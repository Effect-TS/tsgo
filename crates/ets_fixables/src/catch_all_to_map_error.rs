use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_MAP_ERROR_EXPRESSES_THE_SAME_ERROR_TYPE_TRANSFORMATION_MORE_DIRECTLY_THAN_EFFECT_0_FOLLOWED_BY_EFFECT_FAIL_EFFECT_CATCH_ALL_TO_MAP_ERROR;

// go: internal/fixables/catch_all_to_map_error.go:12 CatchAllToMapErrorFix
pub static CATCH_ALL_TO_MAP_ERROR_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "catchAllToMapError",
    description: "Replace Effect.catch + Effect.fail with Effect.mapError",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["catchAllToMapError_fix"],
    run: run_catch_all_to_map_error_fix,
});

// go: internal/fixables/catch_all_to_map_error.go:20 runCatchAllToMapErrorFix
fn run_catch_all_to_map_error_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_catch_all_to_map_error(&mut ctx.type_parser, sf);
    for m in matches {
        let diag_range = m.location;
        if !diag_range.intersects(ctx.span) && !ctx.span.contained_by(diag_range) {
            continue;
        }
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.mapError".to_string(),
            run: Box::new(move |tracker| {
                // Edit 1: Replace "catch" with "mapError" in the callee
                replace_effect_method_callee(
                    tracker,
                    sf,
                    Some(m.callee),
                    m.callee_name_node,
                    "mapError",
                );

                // Edit 2: Unwrap "Effect.fail(arg)" to "arg" by deleting prefix and suffix
                tracker.delete_range(
                    sf,
                    TextRange::from_text_pos(m.fail_call_expression.pos(), m.fail_argument.pos()),
                );
                tracker.delete_range(
                    sf,
                    TextRange::from_text_pos(m.fail_argument.end(), m.fail_call_expression.end()),
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
