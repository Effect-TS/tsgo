use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_EXPRESSION_DISCARDS_THE_SUCCESS_VALUE_THROUGH_MAPPING_EFFECT_AS_VOID_REPRESENTS_THAT_FORM_DIRECTLY_EFFECT_EFFECT_MAP_VOID;

// go: internal/fixables/effect_map_void.go:11 EffectMapVoidFix
pub static EFFECT_MAP_VOID_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "effectMapVoid",
    description: "Replace with Effect.asVoid",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["effectMapVoid_fix"],
    run: run_effect_map_void_fix,
});

// go: internal/fixables/effect_map_void.go:19 runEffectMapVoidFix
//
// Rust: the match's transformation is never nil (a non-optional field), so Go's nil check drops.
fn run_effect_map_void_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_effect_map_void(&mut ctx.type_parser, sf);
    for m in matches {
        let diag_range = m.location;
        if !diag_range.intersects(ctx.span) && !ctx.span.contained_by(diag_range) {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.asVoid".to_string(),
            run: Box::new(move |tracker| {
                // Build Effect.asVoid, preserving the original module reference (and any alias).
                let as_void = effect_module_method(tracker, sf, m.effect_module_node, "asVoid");
                tracker.replace_piping_flow_transformation(
                    Some(sf),
                    Some(&m.transformation),
                    PipingFlowTransformationReplacement {
                        callee: Some(as_void),
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
