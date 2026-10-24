use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::scanner;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::This_arrow_function_block_only_returns_an_expression_and_can_use_a_concise_body_effect_unnecessaryArrowBlock;

// go: internal/fixables/unnecessary_arrow_block.go:13 UnnecessaryArrowBlockFix
pub static UNNECESSARY_ARROW_BLOCK_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "unnecessaryArrowBlock",
    description: "Use a concise arrow body",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["unnecessaryArrowBlock_fix"],
    run: run_unnecessary_arrow_block_fix,
});

// go: internal/fixables/unnecessary_arrow_block.go:21 runUnnecessaryArrowBlockFix
fn run_unnecessary_arrow_block_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    let matches = ets_rules::analyze_unnecessary_arrow_block(&mut ctx.type_parser, sf);

    for m in matches {
        let diag_range = m.location;
        if !diag_range.intersects(ctx.span) && !ctx.span.contained_by(diag_range) {
            continue;
        }

        let replacement_text = "(".to_string()
            + &scanner::get_source_text_of_node_from_source_file(sf, m.returned_expression, false)
            + ")";
        let body_block = m.body_block;
        let ctx_ref: &Context<'_, '_> = ctx;
        if let Some(action) = ctx_ref.new_fix_action(FixAction {
            description: "Use a concise arrow body".to_string(),
            run: Box::new(move |tracker| {
                tracker
                    .delete_range(sf, TextRange::from_text_pos(body_block.pos(), body_block.end()));
                tracker.insert_text(
                    sf,
                    ctx_ref.byte_pos_to_lsp_position(
                        body_block.pos().expect_source("a parsed arrow function body"),
                    ),
                    &replacement_text,
                );
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
