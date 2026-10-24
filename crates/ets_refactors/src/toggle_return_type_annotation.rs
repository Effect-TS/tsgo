use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_shim::ast::{Kind, Node};
use ets_shim::astnav;
use ets_shim::checker::{SignatureKind, Type, TypeFormatFlags};
use ets_shim::core::{P, TextRange};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;
use ets_typeparser::TypeParser;

use crate::*;

// go: internal/refactors/toggle_return_type_annotation.go:14 ToggleReturnTypeAnnotation
pub static TOGGLE_RETURN_TYPE_ANNOTATION: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "toggleReturnTypeAnnotation",
    description: "Toggle return type annotation",
    kind: "rewrite.effect.toggleReturnTypeAnnotation",
    run: run_toggle_return_type_annotation,
});

// go: internal/refactors/toggle_return_type_annotation.go:21 runToggleReturnTypeAnnotation
pub(crate) fn run_toggle_return_type_annotation(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    // Walk ancestor chain looking for a function-like declaration
    let matched_node = std::iter::successors(Some(token), |n| n.parent()).find(|node| {
        matches!(
            node.kind(),
            Kind::FunctionDeclaration
                | Kind::FunctionExpression
                | Kind::ArrowFunction
                | Kind::MethodDeclaration
        )
    });

    let Some(matched_node) = matched_node else {
        return Vec::new();
    };

    // Get the type node and close paren position
    let type_node = get_function_like_type(matched_node);
    let close_paren =
        astnav::find_child_of_kind(matched_node, Kind::CloseParenToken, ctx.source_file);

    // For arrow functions without parens (e.g., x => ...), use the last parameter end
    let mut end_node = None;
    if close_paren.is_some() {
        end_node = close_paren;
    } else if matched_node.kind() == Kind::ArrowFunction {
        let params = matched_node.as_arrow_function().parameters();
        if let Some(params) = params
            && let Some(&last) = params.nodes().last()
        {
            end_node = Some(last);
        }
    }

    let Some(end_node) = end_node else {
        return Vec::new();
    };

    let sf = ctx.source_file;

    if let Some(type_node) = type_node {
        // Remove existing return type annotation: delete from endNode.End() to type.End()
        let Some(mut action) = ctx.new_refactor_action(RefactorAction {
            description: "Toggle return type annotation".to_string(),
            run: Box::new(move |tracker| {
                tracker.delete_range(sf, TextRange::from_text_pos(end_node.end(), type_node.end()));
            }),
        }) else {
            return Vec::new();
        };
        action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.toggleReturnTypeAnnotation");
        return vec![action];
    }

    // Add return type annotation: infer return type and insert after close paren
    // Must have a body to infer from
    if matched_node.body().is_none() {
        return Vec::new();
    }

    let Some(return_type) =
        get_inferred_return_type_from_checker(&mut ctx.type_parser, matched_node)
    else {
        return Vec::new();
    };

    let type_str = ctx.checker().type_to_string_ex_exported(
        return_type,
        Some(matched_node),
        TypeFormatFlags::NoTruncation,
        None,
    );
    if type_str.is_empty() {
        return Vec::new();
    }

    let ctx_ref = &*ctx;
    let Some(mut action) = ctx_ref.new_refactor_action(RefactorAction {
        description: "Toggle return type annotation".to_string(),
        run: Box::new(|tracker| {
            tracker.insert_text(
                sf,
                ctx_ref.byte_pos_to_lsp_position(end_node.end().expect_source("a parsed node")),
                &(": ".to_string() + &type_str),
            );
        }),
    }) else {
        return Vec::new();
    };
    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.toggleReturnTypeAnnotation");
    vec![action]
}

// go: internal/refactors/toggle_return_type_annotation.go:110 getFunctionLikeType
// getFunctionLikeType returns the return type node from a function-like declaration.
pub(crate) fn get_function_like_type(node: P<Node>) -> Option<P<Node>> {
    match node.kind() {
        Kind::FunctionDeclaration => node.as_function_declaration().type_(),
        Kind::FunctionExpression => node.as_function_expression().type_(),
        Kind::ArrowFunction => node.as_arrow_function().type_(),
        Kind::MethodDeclaration => node.as_method_declaration().type_(),
        _ => None,
    }
}

// go: internal/refactors/toggle_return_type_annotation.go:126 getInferredReturnTypeFromChecker
// getInferredReturnTypeFromChecker extracts the return type from a function-like declaration
// using the type checker. It handles overloaded functions and regular signatures.
//
// Rust: Go's `c` is `tp.checker()`.
pub(crate) fn get_inferred_return_type_from_checker(
    tp: &mut TypeParser<'_>,
    declaration: P<Node>,
) -> Option<P<Type>> {
    let mut return_type = None;

    // Try overloaded function handling
    let decl_type = tp.get_type_at_location(declaration);
    if let Some(decl_type) = decl_type {
        let signatures =
            tp.checker().get_signatures_of_type_exported(decl_type, SignatureKind::Call);
        if signatures.len() > 1 {
            let mut return_types = Vec::new();
            for &sig in signatures {
                let rt = tp.checker().get_return_type_of_signature_exported(sig);
                return_types.push(rt);
            }
            if !return_types.is_empty() {
                return_type = Some(tp.checker().get_union_type_exported(&return_types));
            }
        }
    }

    if return_type.is_none() {
        // Go: `GetSignatureFromDeclaration` never returns nil for a function-like declaration.
        let sig = tp.checker().get_signature_from_declaration_exported(declaration);
        return_type = Some(tp.checker().get_return_type_of_signature_exported(sig));
    }

    return_type
}
