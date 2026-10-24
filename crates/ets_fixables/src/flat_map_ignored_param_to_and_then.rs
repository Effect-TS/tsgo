use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::Effect_andThen_expresses_this_sequencing_more_directly_than_Effect_flatMap_with_a_zero_parameter_callback_effect_flatMapIgnoredParamToAndThen;

// go: internal/fixables/flat_map_ignored_param_to_and_then.go:11 FlatMapIgnoredParamToAndThenFix
pub static FLAT_MAP_IGNORED_PARAM_TO_AND_THEN_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "flatMapIgnoredParamToAndThen",
    description: "Replace Effect.flatMap with Effect.andThen",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["flatMapIgnoredParamToAndThen_fix"],
    run: run_flat_map_ignored_param_to_and_then_fix,
});

// go: internal/fixables/flat_map_ignored_param_to_and_then.go:21 runFlatMapIgnoredParamToAndThenFix
fn run_flat_map_ignored_param_to_and_then_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in
        ets_rules::analyze_flat_map_ignored_param_to_and_then(&mut ctx.type_parser, source_file)
    {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.andThen".to_string(),
            run: Box::new(move |tracker| {
                replace_effect_method_callee(
                    tracker,
                    source_file,
                    Some(m.callee),
                    m.callee_name_node,
                    "andThen",
                );
                tracker.replace_node(source_file, Some(m.callback), Some(m.effect_value), None);
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
