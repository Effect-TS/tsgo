use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_AS_SOME_EXPRESSES_WRAPPING_THE_SUCCESS_VALUE_IN_OPTION_SOME_DIRECTLY_EFFECT_MAP_SOME_TO_AS_SOME;

// go: internal/fixables/map_some_to_as_some.go:12 MapSomeToAsSomeFix 1ee35fca02ec6c7f
pub static MAP_SOME_TO_AS_SOME_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "mapSomeToAsSome",
    description: "Replace Effect.map(Option.some) with Effect.asSome",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["mapSomeToAsSome_fix"],
    run: run_map_some_to_as_some_fix,
});

// go: internal/fixables/map_some_to_as_some.go:22 runMapSomeToAsSomeFix b83b5f88ad66a777
//
// Rust: the match's transformation is never nil (a non-optional field), so Go's nil check drops.
fn run_map_some_to_as_some_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_map_some_to_as_some(&mut ctx.type_parser, source_file) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.asSome".to_string(),
            run: Box::new(move |tracker| {
                let as_some = tracker.new_property_access_path(
                    &ets_typeparser::find_effect_module_reference(Some(source_file))
                        .append(&["asSome"]),
                );
                tracker.replace_piping_flow_transformation(
                    Some(source_file),
                    Some(&m.transformation),
                    PipingFlowTransformationReplacement {
                        callee: Some(as_some),
                        type_arguments: None,
                        arguments: None,
                    },
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
