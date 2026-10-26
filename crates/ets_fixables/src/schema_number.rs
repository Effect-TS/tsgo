use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::Kind;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_SCHEMA_NUMBER_API_ACCEPTS_NA_N_INFINITY_AND_INFINITY_USE_0_FOR_FINITE_DOMAIN_NUMBERS_IF_NON_FINITE_VALUES_ARE_INTENTIONAL_DISABLE_THIS_DIAGNOSTIC_FOR_THAT_LINE_EFFECT_SCHEMA_NUMBER;

// go: internal/fixables/schema_number.go:13 SchemaNumberFix
pub static SCHEMA_NUMBER_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "schemaNumber",
    description: "Replace Schema.Number APIs with finite variants",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["schemaNumber_fix"],
    run: run_schema_number_fix,
});

// go: internal/fixables/schema_number.go:21 runSchemaNumberFix
//
// Rust: the match's reference node is never nil.
fn run_schema_number_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    let matches = ets_rules::analyze_schema_number(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if m.reference_node.kind() != Kind::Identifier {
            continue;
        }
        if m.reference_node
            .parent()
            .is_none_or(|parent| parent.kind() != Kind::PropertyAccessExpression)
        {
            continue;
        }

        let reference_node = m.reference_node;
        let replacement = m.replacement_identifier;
        let description = "Replace with ".to_string() + m.replacement;
        let ctx_ref: &Context<'_, '_> = ctx;
        if let Some(action) = ctx_ref.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                tracker.replace_range_with_text(
                    sf,
                    lsproto::Range {
                        start: ctx_ref.byte_pos_to_lsp_position(
                            reference_node.pos().expect_source("a parsed identifier"),
                        ),
                        end: ctx_ref.byte_pos_to_lsp_position(
                            reference_node.end().expect_source("a parsed identifier"),
                        ),
                    },
                    replacement,
                );
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
