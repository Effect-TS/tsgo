use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_VOID_REPRESENTS_THE_SAME_OUTCOME_AS_EFFECT_SUCCEED_UNDEFINED_OR_EFFECT_SUCCEED_VOID_0_EFFECT_EFFECT_SUCCEED_WITH_VOID;

// go: internal/fixables/effect_succeed_with_void.go:13 EffectSucceedWithVoidFix 7377d505f5f97ba8
pub static EFFECT_SUCCEED_WITH_VOID_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "effectSucceedWithVoid",
    description: "Replace with Effect.void",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["effectSucceedWithVoid_fix"],
    run: run_effect_succeed_with_void_fix,
});

// go: internal/fixables/effect_succeed_with_void.go:21 runEffectSucceedWithVoidFix e12d07a9a1491134
fn run_effect_succeed_with_void_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let matches =
        ets_rules::analyze_effect_succeed_with_void(&mut ctx.type_parser, ctx.source_file);
    run_effect_succeed_constant_fix(ctx, matches, "void")
}

// go: internal/fixables/effect_succeed_with_void.go:28 runEffectSucceedConstantFix ddd985586878cb29
// runEffectSucceedConstantFix replaces the constant subject and the Effect.succeed step of a
// match with Effect.<memberName>, keeping any later steps. The fix is offered only when a
// reference to the constant resolves at the call site.
//
// Rust: the fix action's closure owns its `moduleMember` (its predicates are boxed and not `Clone`), so the loop
// builds a second one, equal to `member`, for it.
pub(crate) fn run_effect_succeed_constant_fix(
    ctx: &mut Context<'_, '_>,
    matches: Vec<ets_rules::EffectSucceedConstantMatch>,
    member_name: &'static str,
) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    let member = effect_module_member(member_name);
    for m in matches {
        let diag_range = m.location;
        if !diag_range.intersects(ctx.span) && !ctx.span.contained_by(diag_range) {
            continue;
        }
        let reference = resolve_module_member(
            ctx,
            m.flow.node(),
            &ets_typeparser::find_effect_module_reference(Some(sf)),
            &member,
        );
        let Some(reference) = reference else {
            continue;
        };
        let member = effect_module_member(member_name);
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.".to_string() + member_name,
            run: Box::new(move |tracker| {
                let replacement_node = reference.expression(tracker, &member);
                ast::set_parent_in_children(replacement_node);
                tracker.replace_piping_flow_prefix(
                    Some(sf),
                    Some(&m.flow),
                    1,
                    Some(replacement_node),
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
