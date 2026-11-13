use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::NodeFlags;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_PROVIDE_SERVICE_EFFECT_PROVIDES_THIS_SERVICE_FROM_THE_ACQUIRING_EFFECT_DIRECTLY_INSTEAD_OF_EFFECT_0_FORWARDING_ITS_PARAMETER_TO_EFFECT_PROVIDE_SERVICE_EFFECT_FLAT_MAP_PROVIDE_SERVICE_TO_PROVIDE_SERVICE_EFFECT;

// go: internal/fixables/flat_map_provide_service_to_provide_service_effect.go:13 FlatMapProvideServiceToProvideServiceEffectFix 7bcf9b0c044ef103
pub static FLAT_MAP_PROVIDE_SERVICE_TO_PROVIDE_SERVICE_EFFECT_FIX: LazyLock<Fixable> =
    LazyLock::new(|| Fixable {
        name: "flatMapProvideServiceToProvideServiceEffect",
        description: "Replace Effect.flatMap or Effect.andThen with Effect.provideServiceEffect",
        error_codes: vec![MESSAGE.code()],
        fix_ids: vec!["flatMapProvideServiceToProvideServiceEffect_fix"],
        run: run_flat_map_provide_service_to_provide_service_effect_fix,
    });

// go: internal/fixables/flat_map_provide_service_to_provide_service_effect.go:23 runFlatMapProvideServiceToProvideServiceEffectFix 3ba18b031923166c
//
// Rust: the fix action's closure moves the match and the `moduleMember` (its predicates are boxed and not
// `Clone`); `resolveModuleMember` only borrows it before.
fn run_flat_map_provide_service_to_provide_service_effect_fix(
    ctx: &mut Context<'_, '_>,
) -> Vec<CodeAction> {
    let matches = ets_rules::analyze_flat_map_provide_service_to_provide_service_effect(
        &mut ctx.type_parser,
        ctx.source_file,
    );
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let provide_service_effect = effect_module_member("provideServiceEffect");
        let target = resolve_module_member(
            ctx,
            m.flow.node(),
            &ets_typeparser::find_effect_module_reference(Some(m.source_file)),
            &provide_service_effect,
        );
        let Some(target) = target else {
            continue;
        };
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.provideServiceEffect".to_string(),
            run: Box::new(move |tracker| {
                let self_ = tracker.piping_flow_prefix_expression(
                    Some(&m.inner),
                    m.inner.transformations().len() - 1,
                );
                let acquire = tracker.piping_flow_prefix_expression(Some(&m.flow), m.index);
                let (Some(self_), Some(acquire)) = (self_, acquire) else {
                    return;
                };
                let replacement = tracker.new_call_expression(
                    target.expression(tracker, &provide_service_effect),
                    None,
                    None,
                    tracker.new_node_list(vec![
                        self_,
                        tracker.deep_clone_node(Some(m.key)).unwrap(),
                        acquire,
                    ]),
                    NodeFlags::None,
                );
                tracker.replace_piping_flow_prefix(
                    Some(m.source_file),
                    Some(&m.flow),
                    m.index + 1,
                    Some(replacement),
                );
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
