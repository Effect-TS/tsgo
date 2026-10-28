// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::checker::SignatureKind;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::TypeParser;

// go: internal/rules/global_error_in_effect_catch.go:15 globalErrorCatchApis 64800b2991408691
// globalErrorCatchApis lists the Effect module APIs that have a catch callback parameter.
pub(crate) static GLOBAL_ERROR_CATCH_APIS: &[&str] =
    &["tryPromise", "try", "tryMap", "tryMapPromise"];

// go: internal/rules/global_error_in_effect_catch.go:19 GlobalErrorInEffectCatch 593de55a5367cd8a
// GlobalErrorInEffectCatch detects when catch callbacks in Effect APIs return the global 'Error'
// type instead of providing typed errors.
pub static GLOBAL_ERROR_IN_EFFECT_CATCH: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalErrorInEffectCatch",
    group: "antipattern",
    description: "Warns when catch callbacks return global Error type instead of typed errors",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::THE_CATCH_CALLBACK_IN_0_RETURNS_THE_GLOBAL_ERROR_TYPE_UNTAGGED_ERRORS_MERGE_TOGETHER_IN_THE_EFFECT_ERROR_CHANNEL_AND_LOSE_TYPE_LEVEL_DISTINCTION_A_TAGGED_ERROR_PRESERVES_THAT_DISTINCTION_AND_CAN_WRAP_THE_ORIGINAL_ERROR_IN_A_CAUSE_PROPERTY_EFFECT_GLOBAL_ERROR_IN_EFFECT_CATCH.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(ctx: &mut Context<'_, '_>, n: P<Node>, diags: &mut Vec<P<Diagnostic>>) -> bool {
            if n.kind() == Kind::CallExpression
                && let Some(diag) = check_global_error_in_effect_catch(ctx, n)
            {
                diags.push(diag);
            }

            n.for_each_child(&mut |child| walk(ctx, child, diags));
            false
        }

        walk(ctx, ctx.source_file.as_node(), &mut diags);

        diags
    },
}
});

// go: internal/rules/global_error_in_effect_catch.go:52 checkGlobalErrorInEffectCatch e0731585608477d4
// checkGlobalErrorInEffectCatch checks a single call expression for the global-error-in-catch pattern.
pub(crate) fn check_global_error_in_effect_catch(
    ctx: &mut Context<'_, '_>,
    node: P<Node>,
) -> Option<P<Diagnostic>> {
    if node.kind() != Kind::CallExpression {
        return None;
    }
    let call = node.as_call_expression();

    let callee = call.expression();
    if !is_global_error_catch_callee(ctx.type_parser, callee) {
        return None;
    }

    // Rust: GetResolvedSignature never returns nil (TSRS_API.md).
    let sig = ctx.checker().get_resolved_signature_exported(node);

    let params = sig.parameters();
    if params.is_empty() {
        return None;
    }

    let param_type = ctx.checker().get_type_of_symbol_at_location(params[0], Some(node))?;

    for object_type in ctx.type_parser.unroll_union_members(param_type).unwrap_or_default() {
        let Some(catch_symbol) = ctx.checker().get_property_of_type_exported(object_type, "catch")
        else {
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

        if ctx.type_parser.is_global_error_type(return_type) {
            let callee_text = ets_shim::scanner::get_source_text_of_node_from_source_file(
                ctx.source_file,
                callee,
                false,
            );
            return Some(ctx.new_diagnostic(
                ctx.source_file,
                ctx.get_error_range(callee),
                &tsdiag::THE_CATCH_CALLBACK_IN_0_RETURNS_THE_GLOBAL_ERROR_TYPE_UNTAGGED_ERRORS_MERGE_TOGETHER_IN_THE_EFFECT_ERROR_CHANNEL_AND_LOSE_TYPE_LEVEL_DISTINCTION_A_TAGGED_ERROR_PRESERVES_THAT_DISTINCTION_AND_CAN_WRAP_THE_ORIGINAL_ERROR_IN_A_CAUSE_PROPERTY_EFFECT_GLOBAL_ERROR_IN_EFFECT_CATCH,
                &[],
                &[&callee_text],
            ));
        }
    }

    None
}

// go: internal/rules/global_error_in_effect_catch.go:109 isGlobalErrorCatchCallee 2a7e03ee9342fdf0
// isGlobalErrorCatchCallee checks if a node references one of the Effect module catch APIs.
//
// Rust: Go's unused checker argument is dropped (the parser holds the checker).
pub(crate) fn is_global_error_catch_callee(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    for name in GLOBAL_ERROR_CATCH_APIS {
        if tp.is_node_reference_to_effect_module_api(node, name) {
            return true;
        }
    }
    false
}
