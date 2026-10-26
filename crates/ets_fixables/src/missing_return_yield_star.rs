// Package fixables contains all code fix implementations.

use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_EFFECT_NEVER_SUCCEEDS_USING_RETURN_YIELD_ASTERISK_PRESERVES_A_DEFINITIVE_GENERATOR_EXIT_POINT_FOR_TYPE_NARROWING_AND_TOOLING_SUPPORT_EFFECT_MISSING_RETURN_YIELD_STAR;

// go: internal/fixables/missing_return_yield_star.go:14 MissingReturnYieldStarFix
// MissingReturnYieldStarFix adds "return" before a yield* expression in Effect.gen when needed.
pub static MISSING_RETURN_YIELD_STAR_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "missingReturnYieldStar",
    description: "Add return before yield* for never-success Effect yields",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["missingReturnYieldStar_fix"],
    run: run_missing_return_yield_star_fix,
});

// go: internal/fixables/missing_return_yield_star.go:22 runMissingReturnYieldStarFix
fn run_missing_return_yield_star_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_missing_return_yield_star(&mut ctx.type_parser, sf);

    let mut matched_expr_stmt = None;
    for m in &matches {
        let diag_range = m.location;
        if diag_range.intersects(ctx.span) || ctx.span.contained_by(diag_range) {
            matched_expr_stmt = Some(m.expr_stmt_node);
            break;
        }
    }
    let Some(matched_expr_stmt) = matched_expr_stmt else {
        return Vec::new();
    };

    if let Some(action) = ctx.new_fix_action(FixAction {
        description: "Add return statement".to_string(),
        run: Box::new(move |tracker| {
            let cloned_expr = tracker.deep_clone_node(matched_expr_stmt.expression());
            let return_stmt = tracker.new_return_statement(cloned_expr);
            ast::set_parent_in_children(return_stmt);
            tracker.replace_node(sf, Some(matched_expr_stmt), Some(return_stmt), None);
        }),
    }) {
        return vec![action];
    }
    Vec::new()
}
