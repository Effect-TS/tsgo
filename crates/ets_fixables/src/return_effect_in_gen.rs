// Package fixables contains all code fix implementations.

use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, Kind};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_GENERATOR_RETURNS_AN_EFFECT_ABLE_VALUE_DIRECTLY_WHICH_PRODUCES_A_NESTED_EFFECT_EFFECT_IF_THE_INTENDED_RESULT_IS_THE_INNER_EFFECT_VALUE_RETURN_YIELD_ASTERISK_REPRESENTS_THAT_FORM_EFFECT_RETURN_EFFECT_IN_GEN;

// go: internal/fixables/return_effect_in_gen.go:15 ReturnEffectInGenFix
// ReturnEffectInGenFix adds "yield*" before the return expression in an Effect generator
// when the return value is an Effect-able type (which would cause nested Effect<Effect<...>>).
pub static RETURN_EFFECT_IN_GEN_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "returnEffectInGen",
    description: "Add yield* statement",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["returnEffectInGen_fix"],
    run: run_return_effect_in_gen_fix,
});

// go: internal/fixables/return_effect_in_gen.go:23 runReturnEffectInGenFix
fn run_return_effect_in_gen_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_return_effect_in_gen(&mut ctx.type_parser, sf);

    let mut found = None;
    for m in &matches {
        let diag_range = m.location;
        if diag_range.intersects(ctx.span) || ctx.span.contained_by(diag_range) {
            found = Some(m);
            break;
        }
    }
    let Some(m) = found else {
        return Vec::new();
    };
    let return_node = m.return_node;

    if let Some(action) = ctx.new_fix_action(FixAction {
        description: "Add yield* statement".to_string(),
        run: Box::new(move |tracker| {
            let cloned_expr =
                tracker.deep_clone_node(return_node.as_return_statement().expression());
            let new_yield_expr = tracker
                .new_yield_expression(Some(tracker.new_token(Kind::AsteriskToken)), cloned_expr);
            ast::set_parent_in_children(new_yield_expr);
            tracker.replace_node(
                sf,
                return_node.as_return_statement().expression(),
                Some(new_yield_expr),
                None,
            );
        }),
    }) {
        return vec![action];
    }
    Vec::new()
}
