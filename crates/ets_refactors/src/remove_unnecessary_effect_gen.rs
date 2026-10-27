use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_shim::ast::{self, Kind, NodeFlags};
use ets_shim::astnav;
use ets_shim::core::TextRange;
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

use crate::*;

// go: internal/refactors/remove_unnecessary_effect_gen.go:14 RemoveUnnecessaryEffectGen
pub static REMOVE_UNNECESSARY_EFFECT_GEN: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "removeUnnecessaryEffectGen",
    description: "Remove unnecessary Effect.gen",
    kind: "rewrite.effect.removeUnnecessaryEffectGen",
    run: run_remove_unnecessary_effect_gen,
});

// go: internal/refactors/remove_unnecessary_effect_gen.go:21 runRemoveUnnecessaryEffectGen
pub(crate) fn run_remove_unnecessary_effect_gen(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let matches = ets_rules::analyze_unnecessary_effect_gen(&mut ctx.type_parser, ctx.source_file);
    if matches.is_empty() {
        return Vec::new();
    }

    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    // Walk ancestor chain to find a call expression that matches one of the analyzed nodes
    for node in std::iter::successors(Some(token), |n| n.parent()) {
        if node.kind() != Kind::CallExpression {
            continue;
        }

        for m in &matches {
            if m.call_node != node {
                continue;
            }

            // Found a matching unnecessary Effect.gen call at cursor
            let sf = ctx.source_file;

            if m.explicit_return || m.success_is_void {
                // Simple unwrap: delete the prefix and suffix around the yielded expression
                let Some(mut action) = ctx.new_refactor_action(RefactorAction {
                    description: "Remove unnecessary Effect.gen".to_string(),
                    run: Box::new(|tracker| {
                        tracker.delete_range(
                            sf,
                            TextRange::from_text_pos(m.call_node.pos(), m.yielded_expression.pos()),
                        );
                        tracker.delete_range(
                            sf,
                            TextRange::from_text_pos(m.yielded_expression.end(), m.call_node.end()),
                        );
                    }),
                }) else {
                    return Vec::new();
                };
                action.kind =
                    lsproto::CodeActionKind("refactor.rewrite.effect.removeUnnecessaryEffectGen");
                return vec![action];
            }

            // No return + non-void success: wrap with Effect.asVoid(expr)
            let Some(mut action) = ctx.new_refactor_action(RefactorAction {
                description: "Remove unnecessary Effect.gen".to_string(),
                run: Box::new(|tracker| {
                    let as_void_access = tracker.new_property_access_path(
                        &ets_typeparser::find_effect_module_reference(sf).append(&["asVoid"]),
                    );
                    let cloned_expr = tracker.deep_clone_node(Some(m.yielded_expression)).unwrap();
                    let call_expr = tracker.new_call_expression(
                        as_void_access,
                        None,
                        None,
                        tracker.new_node_list(vec![cloned_expr]),
                        NodeFlags::None,
                    );
                    ast::set_parent_in_children(call_expr);
                    tracker.replace_node(sf, Some(m.call_node), Some(call_expr), None);
                }),
            }) else {
                return Vec::new();
            };
            action.kind =
                lsproto::CodeActionKind("refactor.rewrite.effect.removeUnnecessaryEffectGen");
            return vec![action];
        }
    }

    Vec::new()
}
