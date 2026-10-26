use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::NodeOptions;
use ets_shim::ast::{self, Kind, NodeFlags};
use ets_shim::core::{TextRange, alloc_str};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_LAYER_PROVIDES_0_WHICH_IS_REQUIRED_BY_ANOTHER_LAYER_IN_THE_SAME_LAYER_MERGE_ALL_CALL_LAYER_MERGE_ALL_CREATES_LAYERS_IN_PARALLEL_SO_DEPENDENCIES_BETWEEN_LAYERS_WILL_NOT_BE_SATISFIED_CONSIDER_MOVING_THIS_LAYER_INTO_A_LAYER_PROVIDE_MERGE_AFTER_THE_LAYER_MERGE_ALL_EFFECT_LAYER_MERGE_ALL_WITH_DEPENDENCIES;

// go: internal/fixables/layer_merge_all_with_dependencies.go:14 LayerMergeAllWithDependenciesFix
pub static LAYER_MERGE_ALL_WITH_DEPENDENCIES_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "layerMergeAllWithDependencies",
    description: "Move layer to Layer.provideMerge",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["layerMergeAllWithDependencies_fix"],
    run: run_layer_merge_all_with_dependencies_fix,
});

// go: internal/fixables/layer_merge_all_with_dependencies.go:22 runLayerMergeAllWithDependenciesFix
//
// Rust: the match's call node is never nil (a non-optional field), and tsrs's call expression always has an
// expression, so only Go's kind checks remain.
fn run_layer_merge_all_with_dependencies_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_layer_merge_all_with_dependencies(&mut ctx.type_parser, sf);
    let ctx = &*ctx;
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if m.call_node.kind() != Kind::CallExpression {
            continue;
        }
        let call = m.call_node.as_call_expression();
        let mut layer_module_node = None;
        if call.expression().kind() == Kind::PropertyAccessExpression {
            layer_module_node =
                Some(call.expression().as_property_access_expression().expression());
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Move layer to Layer.provideMerge".to_string(),
            run: Box::new(move |tracker| {
                // Step A: Delete the provider argument from the mergeAll call
                if m.provider_index == 0 && m.all_args.len() > 1 {
                    // First argument with more args: delete from arg start to next arg start
                    tracker.delete_range(
                        sf,
                        TextRange::from_text_pos(m.provider_arg.pos(), m.all_args[1].pos()),
                    );
                } else if m.provider_index > 0 {
                    // Not first argument: delete from previous arg end to this arg end
                    tracker.delete_range(
                        sf,
                        TextRange::from_text_pos(
                            m.all_args[(m.provider_index - 1) as usize].end(),
                            m.provider_arg.end(),
                        ),
                    );
                }

                // Step B: Build Layer.provideMerge(providerArg) call expression
                let cloned_provider_arg = tracker.deep_clone_node(Some(m.provider_arg)).unwrap();
                let cloned_layer_module = match layer_module_node {
                    None => tracker.new_identifier(alloc_str(
                        &ets_typeparser::find_module_identifier(Some(sf), "Layer"),
                    )),
                    Some(layer_module_node) => {
                        tracker.deep_clone_node(Some(layer_module_node)).unwrap()
                    }
                };

                let provide_merge_access = tracker.new_property_access_expression(
                    cloned_layer_module,
                    None,
                    tracker.new_identifier("provideMerge"),
                    NodeFlags::None,
                );

                let provide_merge_call = tracker.new_call_expression(
                    provide_merge_access,
                    None,
                    None,
                    tracker.new_node_list(vec![cloned_provider_arg]),
                    NodeFlags::None,
                );

                ast::set_parent_in_children(provide_merge_call);

                // Step C: Insert .pipe(Layer.provideMerge(...)) at the end of the call
                // Use two separate operations because the Suffix dedup logic in the
                // change tracker skips ")" when the formatted call already ends with ")".
                let call_end = m.call_node.end().expect_source("a parsed call");
                // Rust: `NodeOptions` has crate-private fields, so the literal is built from the default.
                let mut options = NodeOptions::default();
                options.prefix = ".pipe(".to_string();
                tracker.insert_node_at(sf, call_end, provide_merge_call, options);
                tracker.insert_text(sf, ctx.byte_pos_to_lsp_position(call_end), ")");
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
