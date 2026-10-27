// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::EffectContextFlags;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_EFFECT_GENERATOR_CONTAINS_TRY_SLASHCATCH_IN_THIS_CONTEXT_ERROR_HANDLING_IS_EXPRESSED_WITH_EFFECT_AP_IS_SUCH_AS_EFFECT_TRY_EFFECT_TRY_PROMISE_EFFECT_CATCH_EFFECT_CATCH_TAG_EFFECT_TRY_CATCH_IN_EFFECT_GEN;

// go: internal/rules/try_catch_in_effect_gen.go:14 TryCatchInEffectGen efc2587aeb4fce99
// TryCatchInEffectGen detects try/catch statements inside Effect generators
// and suggests using Effect's error handling mechanisms instead.
pub static TRY_CATCH_IN_EFFECT_GEN: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "tryCatchInEffectGen",
    group: "antipattern",
    description: "Discourages try/catch in Effect generators in favor of Effect error handling",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(ctx: &mut Context<'_, '_>, n: P<Node>, diags: &mut Vec<P<Diagnostic>>) -> bool {
            if n.kind() == Kind::TryStatement {
                let try_stmt = n.as_try_statement();
                if try_stmt.catch_clause().is_some()
                    && let Some(diag) = check_try_catch_scope(ctx, n)
                {
                    diags.push(diag);
                }
            }

            n.for_each_child(&mut |child| walk(ctx, child, diags));
            false
        }

        let root = ctx.source_file.as_node();
        walk(ctx, root, &mut diags);
        diags
    },
});

// go: internal/rules/try_catch_in_effect_gen.go:50 checkTryCatchScope 8aef5e98fc5c0e13
// checkTryCatchScope checks if the try statement is directly inside an Effect
// generator scope using FindEnclosingScopes.
pub(crate) fn check_try_catch_scope(
    ctx: &mut Context<'_, '_>,
    try_node: P<Node>,
) -> Option<P<Diagnostic>> {
    if ctx
        .type_parser
        .get_effect_context_flags(try_node)
        .intersects(EffectContextFlags::CanYieldEffect)
    {
        return Some(ctx.new_diagnostic(
            ctx.source_file,
            ctx.get_error_range(try_node),
            MESSAGE,
            &[],
            &[],
        ));
    }
    None
}
