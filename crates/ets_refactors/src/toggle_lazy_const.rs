use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_shim::ast::Kind;
use ets_shim::astnav;
use ets_shim::core::TextRange;
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

use crate::*;

// go: internal/refactors/toggle_lazy_const.go:12 ToggleLazyConst 22aa1074d8647eba
pub static TOGGLE_LAZY_CONST: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "toggleLazyConst",
    description: "Toggle lazy const",
    kind: "rewrite.effect.toggleLazyConst",
    run: run_toggle_lazy_const,
});

// go: internal/refactors/toggle_lazy_const.go:19 runToggleLazyConst f494dbaab79bb6df
pub(crate) fn run_toggle_lazy_const(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let span_pos = span_pos(ctx);
    let token = astnav::get_token_at_position(ctx.source_file, span_pos);

    // Walk ancestor chain looking for a VariableDeclaration
    let mut matched_node = None;
    for node in std::iter::successors(Some(token), |n| n.parent()) {
        if node.kind() != Kind::VariableDeclaration {
            continue;
        }

        let vd = node.as_variable_declaration();

        // Must have an initializer
        let Some(initializer) = vd.initializer() else {
            continue;
        };

        // Cursor must be on the name
        let name = vd.name();
        let name_pos = astnav::get_start_of_node(name, ctx.source_file, false);
        let name_end = name.end().expect_source("a parsed node");
        if span_pos < name_pos || span_pos > name_end {
            continue;
        }

        // Exclude arrow functions with block bodies
        if initializer.kind() == Kind::ArrowFunction {
            let af = initializer.as_arrow_function();
            if af.body().is_some_and(|body| body.kind() == Kind::Block) {
                return Vec::new();
            }
        }

        matched_node = Some(node);
        break;
    }

    let Some(matched_node) = matched_node else {
        return Vec::new();
    };

    let initializer = matched_node.as_variable_declaration().initializer().unwrap();
    let sf = ctx.source_file;

    // Check if this is a zero-param arrow with expression body → unwrap
    if initializer.kind() == Kind::ArrowFunction {
        let af = initializer.as_arrow_function();
        if af.parameters().is_none_or(|params| params.nodes().is_empty())
            && let Some(body) = af.body()
        {
            // Unwrap: remove the `() => ` prefix and any trailing content
            let Some(mut action) = ctx.new_refactor_action(RefactorAction {
                description: "Toggle lazy const".to_string(),
                run: Box::new(move |tracker| {
                    // Delete from body.end to initializer.end (trailing content after body)
                    if body.end() != initializer.end() {
                        tracker.delete_range(
                            sf,
                            TextRange::from_text_pos(body.end(), initializer.end()),
                        );
                    }
                    // Delete from initializer.pos to body.pos (the `() => ` prefix)
                    tracker
                        .delete_range(sf, TextRange::from_text_pos(initializer.pos(), body.pos()));
                }),
            }) else {
                return Vec::new();
            };
            action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.toggleLazyConst");
            return vec![action];
        }
    }

    // Wrap: insert `() => ` before the initializer
    let ctx_ref = &*ctx;
    let Some(mut action) = ctx_ref.new_refactor_action(RefactorAction {
        description: "Toggle lazy const".to_string(),
        run: Box::new(|tracker| {
            tracker.insert_text(
                sf,
                ctx_ref.byte_pos_to_lsp_position(initializer.pos().expect_source("a parsed node")),
                " () =>",
            );
        }),
    }) else {
        return Vec::new();
    };
    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.toggleLazyConst");
    vec![action]
}
