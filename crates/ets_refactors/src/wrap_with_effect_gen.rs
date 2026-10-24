use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_shim::ast::{self, Kind, Node};
use ets_shim::astnav;
use ets_shim::core::P;
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

use crate::*;

// go: internal/refactors/wrap_with_effect_gen.go:13 WrapWithEffectGen
pub static WRAP_WITH_EFFECT_GEN: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "wrapWithEffectGen",
    description: "Wrap with Effect.gen",
    kind: "rewrite.effect.wrapWithEffectGen",
    run: run_wrap_with_effect_gen,
});

// go: internal/refactors/wrap_with_effect_gen.go:20 runWrapWithEffectGen
pub(crate) fn run_wrap_with_effect_gen(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    // Walk ancestor chain looking for an expression whose type is a strict Effect type
    let mut matched_node = None;
    for node in std::iter::successors(Some(token), |n| n.parent()) {
        if !ast::is_expression(node) {
            continue;
        }
        // Skip nodes inside heritage clauses (class extends/implements)
        if is_in_heritage_clause(node) {
            continue;
        }
        // Skip if this is the LHS of a variable declaration (not the initializer)
        if let Some(parent) = node.parent()
            && parent.kind() == Kind::VariableDeclaration
        {
            let var_decl = parent.as_variable_declaration();
            if var_decl.initializer() != Some(node) {
                continue;
            }
        }

        let Some(node_type) = ctx.type_parser.get_type_at_location(node) else {
            continue;
        };
        if !ctx.type_parser.strict_is_effect_type(node_type) {
            continue;
        }
        if ctx.type_parser.effect_gen_call(node).is_some() {
            continue;
        }

        matched_node = Some(node);
        break;
    }

    let Some(matched_node) = matched_node else {
        return Vec::new();
    };

    let effect_module_name = ets_typeparser::find_effect_module_identifier(ctx.source_file);

    let sf = ctx.source_file;
    let ctx_ref = &*ctx;
    let Some(mut action) = ctx_ref.new_refactor_action(RefactorAction {
        description: "Wrap with Effect.gen".to_string(),
        run: Box::new(|tracker| {
            let start = astnav::get_start_of_node(matched_node, sf, false);
            let end = matched_node.end().expect_source("a parsed node");
            let text_range = &sf.text()[start as usize..end as usize];
            let wrapped =
                effect_module_name + ".gen(function*() { return yield* " + text_range + " })";
            tracker.replace_range_with_text(
                sf,
                lsproto::Range {
                    start: ctx_ref.byte_pos_to_lsp_position(start),
                    end: ctx_ref.byte_pos_to_lsp_position(end),
                },
                &wrapped,
            );
        }),
    }) else {
        return Vec::new();
    };

    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.wrapWithEffectGen");
    vec![action]
}

// go: internal/refactors/wrap_with_effect_gen.go:86 isInHeritageClause
// isInHeritageClause checks if a node is inside a heritage clause (extends/implements).
pub(crate) fn is_in_heritage_clause(node: P<Node>) -> bool {
    for parent in std::iter::successors(node.parent(), |n| n.parent()) {
        if parent.kind() == Kind::HeritageClause {
            return true;
        }
        // Stop walking once we hit a statement or declaration boundary
        if parent.kind() == Kind::ClassDeclaration || parent.kind() == Kind::SourceFile {
            return false;
        }
    }
    false
}
