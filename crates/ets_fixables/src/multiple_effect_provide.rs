use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::NodeOptions;
use ets_shim::ast::{self, NodeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::scanner;

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_EXPRESSION_CHAINS_MULTIPLE_EFFECT_PROVIDE_CALLS_PROVIDING_LAYERS_IN_MULTIPLE_CALLS_IN_A_CHAIN_CAN_BREAK_SERVICE_LIFECYCLE_BEHAVIOR_COMPARED_WITH_A_SINGLE_COMBINED_PROVIDE_WITH_MERGED_LAYERS_EFFECT_MULTIPLE_EFFECT_PROVIDE;

// go: internal/fixables/multiple_effect_provide.go:15 MultipleEffectProvideFix
pub static MULTIPLE_EFFECT_PROVIDE_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "multipleEffectProvide",
    description: "Combine into a single provide",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["multipleEffectProvide_fix"],
    run: run_multiple_effect_provide_fix,
});

// go: internal/fixables/multiple_effect_provide.go:23 runMultipleEffectProvideFix
fn run_multiple_effect_provide_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let mut actions = Vec::new();

    let matches = ets_rules::analyze_multiple_effect_provide(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        // Capture loop variables for the closure
        let chunk = m.chunk;
        let layer_args = m.layer_args;

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Combine into a single provide".to_string(),
            run: Box::new(move |tracker| {
                // Step 1: Delete the range spanning all consecutive provide call expressions
                let token_pos = scanner::get_token_pos_of_node(chunk[0], sf, false);
                let end_pos = chunk[chunk.len() - 1].end();
                tracker.delete_range(
                    sf,
                    TextRange::new(token_pos, end_pos.expect_source("a parsed call expression")),
                );

                // Step 2: Build Effect.provide(Layer.mergeAll(arg1, arg2, ...))
                let provide_access = effect_module_method(tracker, sf, "provide");
                let merge_all_access = tracker.new_property_access_path(
                    &ets_typeparser::find_module_reference(sf, "Layer").append(&["mergeAll"]),
                );

                // Clone all layer arguments
                let cloned_args: Vec<_> = layer_args
                    .iter()
                    .map(|&arg| tracker.deep_clone_node(Some(arg)).unwrap())
                    .collect();

                let merge_all_call = tracker.new_call_expression(
                    merge_all_access,
                    None,
                    None,
                    tracker.new_node_list(cloned_args),
                    NodeFlags::None,
                );

                let provide_call = tracker.new_call_expression(
                    provide_access,
                    None,
                    None,
                    tracker.new_node_list(vec![merge_all_call]),
                    NodeFlags::None,
                );

                ast::set_parent_in_children(provide_call);

                // Step 3: Insert the new node at the position of the first deleted call
                tracker.insert_node_at(sf, token_pos, provide_call, NodeOptions::default());
            }),
        }) {
            actions.push(action);
        }
    }

    actions
}
