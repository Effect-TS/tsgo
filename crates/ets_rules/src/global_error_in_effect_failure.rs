// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, FindAncestorResult, Kind, Node};
use ets_shim::diagnostics as tsdiag;

// go: internal/rules/global_error_in_effect_failure.go:14 GlobalErrorInEffectFailure
// GlobalErrorInEffectFailure detects when `new Error(...)` expressions appear inside an Effect
// context where the failure channel (E type parameter) contains the global Error type.
pub static GLOBAL_ERROR_IN_EFFECT_FAILURE: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalErrorInEffectFailure",
    group: "antipattern",
    description: "Warns when the global Error type is used in an Effect failure channel",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::GLOBAL_ERROR_LOSES_TYPE_SAFETY_AS_UNTAGGED_ERRORS_MERGE_TOGETHER_IN_THE_EFFECT_FAILURE_CHANNEL_CONSIDER_USING_A_TAGGED_ERROR_AND_OPTIONALLY_WRAPPING_THE_ORIGINAL_IN_A_CAUSE_PROPERTY_EFFECT_GLOBAL_ERROR_IN_EFFECT_FAILURE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(ctx: &mut Context<'_, '_>, n: P<Node>, diags: &mut Vec<P<Diagnostic>>) -> bool {
            if n.kind() == Kind::NewExpression
                && let Some(diag) = check_global_error_in_effect_failure(ctx, n)
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

// go: internal/rules/global_error_in_effect_failure.go:47 checkGlobalErrorInEffectFailure
// checkGlobalErrorInEffectFailure checks a single new expression for the global-error-in-failure pattern.
pub(crate) fn check_global_error_in_effect_failure(
    ctx: &mut Context<'_, '_>,
    node: P<Node>,
) -> Option<P<Diagnostic>> {
    // Get the type of the new expression
    let new_expr_type = ctx.type_parser.get_type_at_location(node)?;

    // Skip if not a global Error type
    if !ctx.type_parser.is_global_error_type(new_expr_type) {
        return None;
    }

    // Walk up the parent chain to find an enclosing Effect type
    let tp = &mut *ctx.type_parser;
    let ancestor = ast::find_ancestor_or_quit(node.parent(), |current| {
        if tp.node_cannot_be_effect_library_type(current) {
            return FindAncestorResult::False;
        }
        let Some(current_type) = tp.get_type_at_location(current) else {
            return FindAncestorResult::False;
        };

        let Some(effect_type) = tp.effect_type(current_type) else {
            return FindAncestorResult::False;
        };

        // Found an Effect type — check if the failure channel contains global Error
        if tp
            .unroll_union_members(effect_type.e())
            .unwrap_or_default()
            .into_iter()
            .any(|t| tp.is_global_error_type(t))
        {
            return FindAncestorResult::True;
        }

        // Effect type found but failure channel doesn't contain global Error — stop searching
        FindAncestorResult::Quit
    });

    if ancestor.is_some() {
        return Some(ctx.new_diagnostic(
            ctx.source_file,
            ctx.get_error_range(node),
            &tsdiag::GLOBAL_ERROR_LOSES_TYPE_SAFETY_AS_UNTAGGED_ERRORS_MERGE_TOGETHER_IN_THE_EFFECT_FAILURE_CHANNEL_CONSIDER_USING_A_TAGGED_ERROR_AND_OPTIONALLY_WRAPPING_THE_ORIGINAL_IN_A_CAUSE_PROPERTY_EFFECT_GLOBAL_ERROR_IN_EFFECT_FAILURE,
            &[],
            &[],
        ));
    }

    None
}
