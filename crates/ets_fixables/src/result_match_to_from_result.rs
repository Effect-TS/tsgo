use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::ast::{self, NodeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_FROM_RESULT_EXPRESSES_THIS_RESULT_TO_EFFECT_CONVERSION_MORE_DIRECTLY_THAN_RESULT_MATCH_OR_A_RESULT_GUARD_CONDITIONAL_EFFECT_RESULT_MATCH_TO_FROM_RESULT;

// go: internal/fixables/result_match_to_from_result.go:13 ResultMatchToFromResultFix f7833ea9f3d5cc10
pub static RESULT_MATCH_TO_FROM_RESULT_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "resultMatchToFromResult",
    description: "Replace Result.match or a Result guard conditional with Effect.fromResult",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["resultMatchToFromResult_fix"],
    run: run_result_match_to_from_result_fix,
});

// go: internal/fixables/result_match_to_from_result.go:23 runResultMatchToFromResultFix 9c8436c9f3bd87f9
fn run_result_match_to_from_result_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_result_match_to_from_result(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if m.transformation.is_none() && (m.replacement_node.is_none() || m.result_node.is_none()) {
            continue;
        }
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.fromResult".to_string(),
            run: Box::new(move |tracker| {
                let callee = tracker.new_property_access_path(
                    &ets_typeparser::find_effect_module_reference(Some(sf)).append(&["fromResult"]),
                );
                if let Some(transformation) = &m.transformation {
                    tracker.replace_piping_flow_transformation(
                        Some(sf),
                        Some(transformation),
                        PipingFlowTransformationReplacement {
                            callee: Some(callee),
                            ..Default::default()
                        },
                    );
                    return;
                }

                let replacement = tracker.new_call_expression(
                    callee,
                    None,
                    None,
                    tracker.new_node_list(vec![tracker.deep_clone_node(m.result_node).unwrap()]),
                    NodeFlags::None,
                );
                ast::set_parent_in_children(replacement);
                tracker.replace_node(sf, m.replacement_node, Some(replacement), None);
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
