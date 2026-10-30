use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;
use ets_shim::scanner;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_PROVIDE_ACCEPTS_AN_ARRAY_OF_LAYERS_AND_MERGES_THEM_ITSELF_PASS_THE_LAYERS_AS_AN_ARRAY_INSTEAD_OF_WRAPPING_THEM_IN_LAYER_MERGE_ALL_EFFECT_PROVIDE_MERGE_ALL_TO_ARRAY;

// go: internal/fixables/provide_merge_all_to_array.go:13 ProvideMergeAllToArrayFix 951e8fb72894729c
pub static PROVIDE_MERGE_ALL_TO_ARRAY_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "provideMergeAllToArray",
    description: "Pass the layers as an array to Effect.provide",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["provideMergeAllToArray_fix"],
    run: run_provide_merge_all_to_array_fix,
});

// go: internal/fixables/provide_merge_all_to_array.go:23 runProvideMergeAllToArrayFix a18610d4b75cf6c6
fn run_provide_merge_all_to_array_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    let text = sf.text();
    for m in ets_rules::analyze_provide_merge_all_to_array(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        // Only the call syntax around the layers is replaced, so the layer
        // expressions, spreads, comments and trailing comma stay as written.
        let arguments = m.merge_all.as_call_expression().arguments();
        let open_start = scanner::get_token_pos_of_node(m.argument, sf, false);
        let close_start =
            scanner::skip_trivia(text, arguments.end().expect_source("a parsed argument list"));
        if close_start as usize >= text.len() || text.as_bytes()[close_start as usize] != b')' {
            continue;
        }
        let ctx_ref: &Context<'_, '_> = ctx;
        if let Some(action) = ctx_ref.new_fix_action(FixAction {
            description: "Pass the layers as an array to Effect.provide".to_string(),
            run: Box::new(move |tracker| {
                tracker.replace_range_with_text(
                    sf,
                    lsproto::Range {
                        start: ctx_ref.byte_pos_to_lsp_position(open_start),
                        end: ctx_ref.byte_pos_to_lsp_position(
                            arguments.pos().expect_source("a parsed argument list"),
                        ),
                    },
                    "[",
                );
                tracker.replace_range_with_text(
                    sf,
                    lsproto::Range {
                        start: ctx_ref.byte_pos_to_lsp_position(close_start),
                        end: ctx_ref.byte_pos_to_lsp_position(
                            m.argument.end().expect_source("a parsed provide argument"),
                        ),
                    },
                    "]",
                );
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
