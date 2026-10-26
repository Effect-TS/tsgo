use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::scanner;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::IDENTIFIER_0_IS_REDUNDANT_SINCE_IT_EQUALS_THE_TAG_VALUE_EFFECT_REDUNDANT_SCHEMA_TAG_IDENTIFIER;

// go: internal/fixables/redundant_schema_tag_identifier.go:15 RedundantSchemaTagIdentifierRemoveIdentifierFix
pub static REDUNDANT_SCHEMA_TAG_IDENTIFIER_REMOVE_IDENTIFIER_FIX: LazyLock<Fixable> =
    LazyLock::new(|| Fixable {
        name: "redundantSchemaTagIdentifier",
        description: "Remove redundant identifier",
        error_codes: vec![MESSAGE.code()],
        fix_ids: vec!["redundantSchemaTagIdentifier_removeIdentifier"],
        run: run_redundant_schema_tag_identifier_fix,
    });

// go: internal/fixables/redundant_schema_tag_identifier.go:23 runRedundantSchemaTagIdentifierFix
fn run_redundant_schema_tag_identifier_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_redundant_schema_tag_identifier(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        let key_text = m.key_string_literal.as_string_literal().text();
        let key_node = m.key_string_literal;

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: format!("Remove redundant identifier '{key_text}'"),
            run: Box::new(move |tracker| {
                let token_pos = scanner::get_token_pos_of_node(key_node, sf, false);
                tracker.delete_range(
                    sf,
                    TextRange::new(
                        token_pos,
                        key_node.end().expect_source("a parsed string literal"),
                    ),
                );
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
