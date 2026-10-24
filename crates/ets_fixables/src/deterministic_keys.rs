use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::TokenFlags;
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::This_key_does_not_match_the_deterministic_key_for_this_declaration_The_expected_key_is_0_effect_deterministicKeys;

// go: internal/fixables/deterministic_keys.go:12 DeterministicKeysFix
pub static DETERMINISTIC_KEYS_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "deterministicKeys",
    description: "Replace key with expected deterministic key",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["deterministicKeys_fix"],
    run: run_deterministic_keys_fix,
});

// go: internal/fixables/deterministic_keys.go:20 runDeterministicKeysFix
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
