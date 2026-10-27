use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::Kind;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_INPUT_IS_ALREADY_ASSIGNABLE_TO_THE_SCHEMA_S_ENCODED_TYPE_USE_0_TO_PRESERVE_COMPILE_TIME_TYPE_CHECKING_INSTEAD_OF_DISCARDING_THE_INPUT_TYPE_THROUGH_1_EFFECT_PREFER_TYPED_SCHEMA_DECODER;

// go: internal/fixables/prefer_typed_schema_decoder.go:12 PreferTypedSchemaDecoderFix d2e9416fb3f10b32
pub static PREFER_TYPED_SCHEMA_DECODER_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "preferTypedSchemaDecoder",
    description: "Replace the unknown-input Schema decoder with its typed variant",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["preferTypedSchemaDecoder_fix"],
    run: run_prefer_typed_schema_decoder_fix,
});

// go: internal/fixables/prefer_typed_schema_decoder.go:22 runPreferTypedSchemaDecoderFix e68d144801affba2
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
