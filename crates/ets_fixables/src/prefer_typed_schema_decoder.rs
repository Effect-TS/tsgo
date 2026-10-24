use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::Kind;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::This_input_is_already_assignable_to_the_schema_s_Encoded_type_Use_0_to_preserve_compile_time_type_checking_instead_of_discarding_the_input_type_through_1_effect_preferTypedSchemaDecoder;

// go: internal/fixables/prefer_typed_schema_decoder.go:12 PreferTypedSchemaDecoderFix
pub static PREFER_TYPED_SCHEMA_DECODER_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "preferTypedSchemaDecoder",
    description: "Replace the unknown-input Schema decoder with its typed variant",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["preferTypedSchemaDecoder_fix"],
    run: run_prefer_typed_schema_decoder_fix,
});

// go: internal/fixables/prefer_typed_schema_decoder.go:22 runPreferTypedSchemaDecoderFix
fn run_prefer_typed_schema_decoder_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_prefer_typed_schema_decoder(&mut ctx.type_parser, sf) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if m.decoder_name
            .parent()
            .is_none_or(|parent| parent.kind() != Kind::PropertyAccessExpression)
        {
            continue;
        }
        let decoder_name = m.decoder_name;
        let typed_name = m.typed_name;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with ".to_string() + typed_name,
            run: Box::new(move |tracker| {
                let identifier = tracker.new_identifier(typed_name);
                tracker.replace_node(sf, Some(decoder_name), Some(identifier), None);
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
