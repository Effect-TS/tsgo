use std::sync::LazyLock;

use ets_fixable::{Context, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_UNDEFINED_REPRESENTS_THE_SAME_OUTCOME_AS_EFFECT_SUCCEED_UNDEFINED_OR_EFFECT_SUCCEED_VOID_0_EFFECT_EFFECT_SUCCEED_WITH_UNDEFINED;

// go: internal/fixables/effect_succeed_with_undefined.go:10 EffectSucceedWithUndefinedFix 44d7df8b70177da8
pub static EFFECT_SUCCEED_WITH_UNDEFINED_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "effectSucceedWithUndefined",
    description: "Replace with Effect.undefined",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["effectSucceedWithUndefined_fix"],
    run: run_effect_succeed_with_undefined_fix,
});

// go: internal/fixables/effect_succeed_with_undefined.go:18 runEffectSucceedWithUndefinedFix b6efb417c69cf2d6
fn run_effect_succeed_with_undefined_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let matches =
        ets_rules::analyze_effect_succeed_with_undefined(&mut ctx.type_parser, ctx.source_file);
    run_effect_succeed_constant_fix(ctx, matches, "undefined")
}
