use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::ast;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_FLATTEN_EXPRESSES_THIS_UNWRAPPING_MORE_DIRECTLY_THAN_EFFECT_FLAT_MAP_WITH_AN_IDENTITY_CALLBACK_EFFECT_FLAT_MAP_IDENTITY_TO_FLATTEN;

// go: internal/fixables/flat_map_identity_to_flatten.go:13 FlatMapIdentityToFlattenFix 4711c9ae598004b0
pub static FLAT_MAP_IDENTITY_TO_FLATTEN_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "flatMapIdentityToFlatten",
    description: "Replace Effect.flatMap with an identity callback by Effect.flatten",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["flatMapIdentityToFlatten_fix"],
    run: run_flat_map_identity_to_flatten_fix,
});

// go: internal/fixables/flat_map_identity_to_flatten.go:23 runFlatMapIdentityToFlattenFix e7383ab6838b588c
fn run_flat_map_identity_to_flatten_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_flat_map_identity_to_flatten(&mut ctx.type_parser, source_file) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        // Only offer the fix when the step is called through an Effect module receiver.
        if m.effect_module_node.is_none() {
            continue;
        }
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.flatten".to_string(),
            run: Box::new(move |tracker| {
                let flatten = tracker.new_property_access_path(
                    &ets_typeparser::find_effect_module_reference(Some(m.source_file))
                        .append(&["flatten"]),
                );
                ast::set_parent_in_children(flatten);
                tracker.replace_piping_flow_transformation(
                    Some(m.source_file),
                    Some(&m.transformation),
                    PipingFlowTransformationReplacement {
                        callee: Some(flatten),
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
