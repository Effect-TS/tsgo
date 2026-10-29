use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, Kind};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::EffectContextFlags;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE_1: &Message =
    &tsdiag::THIS_EFFECT_VALUE_IS_NEITHER_YIELDED_NOR_USED_IN_AN_ASSIGNMENT_EFFECT_FLOATING_EFFECT;
/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE_2: &Message = &tsdiag::THIS_EFFECT_ABLE_0_VALUE_IS_NEITHER_YIELDED_NOR_ASSIGNED_TO_A_VARIABLE_EFFECT_FLOATING_EFFECT;

// go: internal/fixables/floating_effect_yield.go:14 FloatingEffectYieldFix fd78cf0499c400a3
// FloatingEffectYieldFix yields a floating Effect when it occurs in an Effect generator.
pub static FLOATING_EFFECT_YIELD_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "floatingEffectYield",
    description: "Add yield* statement",
    error_codes: vec![MESSAGE_1.code(), MESSAGE_2.code()],
    fix_ids: vec!["floatingEffectYield_fix"],
    run: run_floating_effect_yield_fix,
});

// go: internal/fixables/floating_effect_yield.go:25 runFloatingEffectYieldFix 56d1bb9284363981
fn run_floating_effect_yield_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_floating_effect(&mut ctx.type_parser, source_file) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if !ctx
            .type_parser
            .get_effect_context_flags(m.expression)
            .intersects(EffectContextFlags::CanYieldEffect)
        {
            return Vec::new();
        }
        if ctx.type_parser.effect_yieldable_type_at_location(m.expression).is_none() {
            return Vec::new();
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Add yield* statement".to_string(),
            run: Box::new(move |tracker| {
                let cloned_expr = tracker.deep_clone_node(Some(m.expression));
                let yield_expr = tracker.new_yield_expression(
                    Some(tracker.new_token(Kind::AsteriskToken)),
                    cloned_expr,
                );
                ast::set_parent_in_children(yield_expr);
                tracker.replace_node(source_file, Some(m.expression), Some(yield_expr), None);
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }
    Vec::new()
}
