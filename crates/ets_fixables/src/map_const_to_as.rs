use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message =
    &tsdiag::EFFECT_AS_EXPRESSES_REPLACING_THE_SUCCESS_VALUE_WITH_A_CONSTANT_DIRECTLY_EFFECT_MAP_CONST_TO_AS;

// go: internal/fixables/map_const_to_as.go:13 MapConstToAsFix
pub static MAP_CONST_TO_AS_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "mapConstToAs",
    description: "Replace Effect.map with Effect.as",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["mapConstToAs_fix"],
    run: run_map_const_to_as_fix,
});

// go: internal/fixables/map_const_to_as.go:23 runMapConstToAsFix
//
// Rust: the match's transformation is never nil (a non-optional field), so Go's nil check drops.
fn run_map_const_to_as_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_map_const_to_as(&mut ctx.type_parser, source_file) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.as".to_string(),
            run: Box::new(move |tracker| {
                let as_ = tracker.new_property_access_path(
                    &ets_typeparser::find_effect_module_reference(Some(source_file))
                        .append(&["as"]),
                );
                let arguments = tracker
                    .new_node_list(vec![tracker.deep_clone_node(Some(m.constant_value)).unwrap()]);
                tracker.replace_piping_flow_transformation(
                    Some(source_file),
                    Some(&m.transformation),
                    PipingFlowTransformationReplacement {
                        callee: Some(as_),
                        type_arguments: None,
                        arguments: Some(arguments),
                    },
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
