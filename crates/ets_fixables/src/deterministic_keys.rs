use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::TokenFlags;
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_KEY_DOES_NOT_MATCH_THE_DETERMINISTIC_KEY_FOR_THIS_DECLARATION_THE_EXPECTED_KEY_IS_0_EFFECT_DETERMINISTIC_KEYS;

// go: internal/fixables/deterministic_keys.go:12 DeterministicKeysFix 9fa1de8a2da87524
pub static DETERMINISTIC_KEYS_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "deterministicKeys",
    description: "Replace key with expected deterministic key",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["deterministicKeys_fix"],
    run: run_deterministic_keys_fix,
});

// go: internal/fixables/deterministic_keys.go:20 runDeterministicKeysFix 3751401c84c0d71a
fn run_deterministic_keys_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let matches = ets_rules::analyze_deterministic_keys(
        &mut ctx.type_parser,
        ctx.program,
        ctx.source_file,
        ctx.options,
    );
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        // Determine the quote style from the original string literal
        let mut flags = TokenFlags::None;
        if m.key_string_literal.as_string_literal().token_flags().contains(TokenFlags::SingleQuote)
        {
            flags = TokenFlags::SingleQuote;
        }

        let description = format!("Replace '{}' with '{}'", m.actual_key, m.expected_key);
        let sf = ctx.source_file;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                let replacement = tracker.new_string_literal(alloc_str(&m.expected_key), flags);
                tracker.replace_node(sf, Some(m.key_string_literal), Some(replacement), None);
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
