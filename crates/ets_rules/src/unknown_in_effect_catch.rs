// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::checker::{SignatureKind, TypeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THE_CATCH_CALLBACK_IN_0_RETURNS_UNKNOWN_SO_THE_EFFECT_ERROR_TYPE_STAYS_UNTYPED_A_SPECIFIC_TYPED_ERROR_PRESERVES_ERROR_CHANNEL_INFORMATION_FOR_EXAMPLE_BY_NARROWING_THE_VALUE_OR_WRAPPING_IT_IN_DATA_TAGGED_ERROR_EFFECT_UNKNOWN_IN_EFFECT_CATCH;

// go: internal/rules/unknown_in_effect_catch.go:15 unknownCatchApis
// unknownCatchApis lists the Effect module APIs that have a catch callback parameter.
pub(crate) static UNKNOWN_CATCH_APIS: &[&str] = &["tryPromise", "try", "tryMap", "tryMapPromise"];

// go: internal/rules/unknown_in_effect_catch.go:19 UnknownInEffectCatch
// UnknownInEffectCatch detects when catch callbacks in Effect APIs return 'unknown' or 'any'
// instead of providing typed errors.
pub static UNKNOWN_IN_EFFECT_CATCH: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "unknownInEffectCatch",
    group: "antipattern",
    description: "Warns when catch callbacks return unknown instead of typed errors",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(ctx: &mut Context<'_, '_>, n: P<Node>, diags: &mut Vec<P<Diagnostic>>) -> bool {
            if n.kind() == Kind::CallExpression
                && let Some(diag) = check_unknown_in_effect_catch(ctx, n)
            {
                diags.push(diag);
            }

            n.for_each_child(&mut |child| walk(ctx, child, diags));
            false
        }

        let root = ctx.source_file.as_node();
        walk(ctx, root, &mut diags);

        diags
    },
});

// go: internal/rules/unknown_in_effect_catch.go:52 checkUnknownInEffectCatch
// checkUnknownInEffectCatch checks a single call expression for the unknown-in-catch pattern.
pub(crate) fn check_unknown_in_effect_catch(
    ctx: &mut Context<'_, '_>,
    node: P<Node>,
) -> Option<P<Diagnostic>> {
    if node.kind() != Kind::CallExpression {
        return None;
    }
    let call = node.as_call_expression();

    let callee = call.expression();
    if !is_unknown_catch_callee(ctx.type_parser, callee) {
        return None;
    }

    let sig = ctx.checker().get_resolved_signature_exported(node);

    let params = sig.parameters();
    if params.is_empty() {
        return None;
    }

    let param_type = ctx.checker().get_type_of_symbol_at_location(params[0], Some(node))?;

    for object_type in ctx.type_parser.unroll_union_members(param_type).unwrap_or_default() {
        let Some(catch_symbol) = ctx.checker().get_property_of_type(object_type, "catch") else {
            continue;
        };

        let Some(catch_type) =
            ctx.checker().get_type_of_symbol_at_location(catch_symbol, Some(node))
        else {
            continue;
        };

        let signatures = ctx.checker().get_signatures_of_type(catch_type, SignatureKind::Call);
        if signatures.is_empty() {
            continue;
        }

        let return_type = ctx.checker().get_return_type_of_signature(signatures[0]);

        if return_type.flags().intersects(TypeFlags::Unknown | TypeFlags::Any) {
            let callee_text = ets_shim::scanner::get_source_text_of_node_from_source_file(
                ctx.source_file,
                callee,
                false,
            );
            return Some(ctx.new_diagnostic(
                ctx.source_file,
                ctx.get_error_range(callee),
                MESSAGE,
                &[],
                &[&callee_text],
            ));
        }
    }

    None
}

// go: internal/rules/unknown_in_effect_catch.go:109 isUnknownCatchCallee
// isUnknownCatchCallee checks if a node references one of the Effect module catch APIs.
//
// Rust: Go's ignored checker argument is dropped.
pub(crate) fn is_unknown_catch_callee(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    for name in UNKNOWN_CATCH_APIS {
        if tp.is_node_reference_to_effect_module_api(node, name) {
            return true;
        }
    }
    false
}
