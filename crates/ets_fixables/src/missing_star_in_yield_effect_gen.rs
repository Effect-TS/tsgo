use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, Kind};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_USES_YIELD_FOR_AN_EFFECT_VALUE_YIELD_ASTERISK_IS_THE_EFFECT_AWARE_FORM_IN_THIS_CONTEXT_EFFECT_MISSING_STAR_IN_YIELD_EFFECT_GEN;

// go: internal/fixables/missing_star_in_yield_effect_gen.go:12 MissingStarInYieldEffectGenFix 269059e400eb583b
pub static MISSING_STAR_IN_YIELD_EFFECT_GEN_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "missingStarInYieldEffectGen",
    description: "Replace yield with yield* inside Effect generator scopes",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["missingStarInYieldEffectGen_fix"],
    run: run_missing_star_in_yield_effect_gen_fix,
});

// go: internal/fixables/missing_star_in_yield_effect_gen.go:20 runMissingStarInYieldEffectGenFix 2a677891e2110f50
fn run_missing_star_in_yield_effect_gen_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_missing_star_in_yield_effect_gen(&mut ctx.type_parser, sf);

    let mut yield_node = None;
    for m in &matches {
        let diag_range = m.location;
        if diag_range.intersects(ctx.span) || ctx.span.contained_by(diag_range) {
            yield_node = Some(m.yield_node);
            break;
        }
    }
    let Some(yield_node) = yield_node else {
        return Vec::new();
    };

    if let Some(action) = ctx.new_fix_action(FixAction {
        description: "Replace yield with yield*".to_string(),
        run: Box::new(move |tracker| {
            let cloned_expr =
                tracker.deep_clone_node(yield_node.as_yield_expression().expression());
            let new_yield_expr = tracker
                .new_yield_expression(Some(tracker.new_token(Kind::AsteriskToken)), cloned_expr);
            ast::set_parent_in_children(new_yield_expr);
            tracker.replace_node(sf, Some(yield_node), Some(new_yield_expr), None);
        }),
    }) {
        return vec![action];
    }
    Vec::new()
}
