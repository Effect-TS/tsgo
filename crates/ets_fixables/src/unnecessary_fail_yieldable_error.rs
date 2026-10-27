use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_YIELD_ASTERISK_EFFECT_FAIL_PASSES_A_YIELDABLE_ERROR_VALUE_YIELD_ASTERISK_REPRESENTS_THAT_VALUE_DIRECTLY_WITHOUT_WRAPPING_IT_IN_EFFECT_FAIL_EFFECT_UNNECESSARY_FAIL_YIELDABLE_ERROR;

// go: internal/fixables/unnecessary_fail_yieldable_error.go:12 UnnecessaryFailYieldableErrorFix 76f54507b43ca1eb
pub static UNNECESSARY_FAIL_YIELDABLE_ERROR_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "unnecessaryFailYieldableError",
    description: "Replace yield* Effect.fail with yield*",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["unnecessaryFailYieldableError_fix"],
    run: run_unnecessary_fail_yieldable_error_fix,
});

// go: internal/fixables/unnecessary_fail_yieldable_error.go:20 runUnnecessaryFailYieldableErrorFix 859be82c872a9b02
fn run_unnecessary_fail_yieldable_error_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_unnecessary_fail_yieldable_error(&mut ctx.type_parser, sf);
    for m in matches {
        let diag_range = m.location;
        if !diag_range.intersects(ctx.span) && !ctx.span.contained_by(diag_range) {
            continue;
        }

        // Unwrap "Effect.fail(arg)" to just "arg" by deleting the prefix and suffix around the argument.
        // This keeps "yield*" in place, changing "yield* Effect.fail(error)" to "yield* error".
        let call_node = m.call_node;
        let fail_argument = m.fail_argument;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace yield* Effect.fail with yield*".to_string(),
            run: Box::new(move |tracker| {
                tracker.delete_range(
                    sf,
                    TextRange::from_text_pos(call_node.pos(), fail_argument.pos()),
                );
                tracker.delete_range(
                    sf,
                    TextRange::from_text_pos(fail_argument.end(), call_node.end()),
                );
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
