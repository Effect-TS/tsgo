use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_PROVIDES_THIS_INLINE_SINGLE_SERVICE_LAYER_DIRECTLY_EFFECT_PROVIDE_LAYER_SUCCEED_TO_PROVIDE_SERVICE;

// go: internal/fixables/provide_layer_succeed_to_provide_service.go:12 ProvideLayerSucceedToProvideServiceFix
pub static PROVIDE_LAYER_SUCCEED_TO_PROVIDE_SERVICE_FIX: LazyLock<Fixable> =
    LazyLock::new(|| Fixable {
        name: "provideLayerSucceedToProvideService",
        description: "Replace an inline single-service layer with direct service provision",
        error_codes: vec![MESSAGE.code()],
        fix_ids: vec!["provideLayerSucceedToProvideService_fix"],
        run: run_provide_layer_succeed_to_provide_service_fix,
    });

// go: internal/fixables/provide_layer_succeed_to_provide_service.go:22 runProvideLayerSucceedToProvideServiceFix
//
// Rust: the match's transformation is never nil (an owned copy), so Go's nil check drops. The implementation node
// is the inline layer's piping subject, which the rule requires to be set.
fn run_provide_layer_succeed_to_provide_service_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in
        ets_rules::analyze_provide_layer_succeed_to_provide_service(&mut ctx.type_parser, Some(sf))
    {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.".to_string() + m.replacement_method_name,
            run: Box::new(move |tracker| {
                let callee = effect_module_method(
                    tracker,
                    sf,
                    m.effect_module_node,
                    m.replacement_method_name,
                );
                let arguments = tracker.new_node_list(vec![
                    tracker.deep_clone_node(Some(m.service_node)).unwrap(),
                    tracker
                        .deep_clone_node(m.implementation_node)
                        .expect("the inline layer's subject is set"),
                ]);
                tracker.replace_piping_flow_transformation(
                    Some(sf),
                    Some(&m.provide_transformation),
                    PipingFlowTransformationReplacement {
                        callee: Some(callee),
                        arguments: Some(arguments),
                        ..Default::default()
                    },
                );
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
