use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_AND_THEN_EXPRESSES_THIS_SEQUENCING_MORE_DIRECTLY_THAN_EFFECT_FLAT_MAP_WITH_A_ZERO_PARAMETER_CALLBACK_EFFECT_FLAT_MAP_IGNORED_PARAM_TO_AND_THEN;

// go: internal/fixables/flat_map_ignored_param_to_and_then.go:12 FlatMapIgnoredParamToAndThenFix e3d5953cc091eb1f
pub static FLAT_MAP_IGNORED_PARAM_TO_AND_THEN_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "flatMapIgnoredParamToAndThen",
    description: "Replace Effect.flatMap with Effect.andThen",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["flatMapIgnoredParamToAndThen_fix"],
    run: run_flat_map_ignored_param_to_and_then_fix,
});

// go: internal/fixables/flat_map_ignored_param_to_and_then.go:22 runFlatMapIgnoredParamToAndThenFix e8b7e8e38caffb03
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
                tracker.replace_node(
                    source_file,
                    Some(m.callee),
                    Some(
                        tracker.new_property_access_path(
                            &ets_typeparser::find_effect_module_reference(Some(source_file))
                                .append(&["andThen"]),
                        ),
                    ),
                    None,
                );
                tracker.replace_node(source_file, Some(m.callback), Some(m.effect_value), None);
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
