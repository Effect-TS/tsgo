use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_shim::ast::Kind;
use ets_shim::astnav;
use ets_shim::checker::TypeFormatFlags;
use ets_shim::core::TextRange;
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

use crate::*;

// go: internal/refactors/toggle_type_annotation.go:13 ToggleTypeAnnotation
pub static TOGGLE_TYPE_ANNOTATION: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "toggleTypeAnnotation",
    description: "Toggle type annotation",
    kind: "rewrite.effect.toggleTypeAnnotation",
    run: run_toggle_type_annotation,
});

// go: internal/refactors/toggle_type_annotation.go:20 runToggleTypeAnnotation
pub(crate) fn run_toggle_type_annotation(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let span_pos = span_pos(ctx);
    let token = astnav::get_token_at_position(ctx.source_file, span_pos);

    // Walk ancestor chain looking for a VariableDeclaration or PropertyDeclaration
    let mut matched_node = None;
    for node in std::iter::successors(Some(token), |n| n.parent()) {
        if node.kind() != Kind::VariableDeclaration && node.kind() != Kind::PropertyDeclaration {
            continue;
        }

        // Must have an initializer
        let (initializer, name) = if node.kind() == Kind::VariableDeclaration {
            let vd = node.as_variable_declaration();
            (vd.initializer(), vd.name())
        } else {
            let pd = node.as_property_declaration();
            (pd.initializer(), pd.name())
        };

        if initializer.is_none() {
            continue;
        }

        // Cursor must be on the name
        let name_pos = astnav::get_start_of_node(name, ctx.source_file, false);
        let name_end = name.end().expect_source("a parsed node");
        if span_pos < name_pos || span_pos > name_end {
            continue;
        }

        matched_node = Some(node);
        break;
    }

    let Some(matched_node) = matched_node else {
        return Vec::new();
    };

    // Get name and type nodes
    let (name, type_node, initializer) = if matched_node.kind() == Kind::VariableDeclaration {
        let vd = matched_node.as_variable_declaration();
        (vd.name(), vd.type_(), vd.initializer())
    } else {
        let pd = matched_node.as_property_declaration();
        (pd.name(), pd.type_(), pd.initializer())
    };

    let sf = ctx.source_file;

    if let Some(type_node) = type_node {
        // Remove existing type annotation: delete from name.End() to type.End()
        let Some(mut action) = ctx.new_refactor_action(RefactorAction {
            description: "Toggle type annotation".to_string(),
            run: Box::new(move |tracker| {
                tracker.delete_range(sf, TextRange::from_text_pos(name.end(), type_node.end()));
            }),
        }) else {
            return Vec::new();
        };
        action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.toggleTypeAnnotation");
        return vec![action];
    }

    // Add type annotation: infer type from initializer and insert after name
    let Some(initializer_type) = ctx.type_parser.get_type_at_location(initializer) else {
        return Vec::new();
    };

    let type_str = ctx.checker().type_to_string_ex_exported(
        initializer_type,
        Some(matched_node),
        TypeFormatFlags::NoTruncation,
        None,
    );
    if type_str.is_empty() {
        return Vec::new();
    }

    let ctx_ref = &*ctx;
    let Some(mut action) = ctx_ref.new_refactor_action(RefactorAction {
        description: "Toggle type annotation".to_string(),
        run: Box::new(|tracker| {
            tracker.insert_text(
                sf,
                ctx_ref.byte_pos_to_lsp_position(name.end().expect_source("a parsed node")),
                &(": ".to_string() + &type_str),
            );
        }),
    }) else {
        return Vec::new();
    };
    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.toggleTypeAnnotation");
    vec![action]
}
