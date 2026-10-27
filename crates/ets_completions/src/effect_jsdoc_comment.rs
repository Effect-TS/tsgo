use std::sync::LazyLock;

use ets_completion::{Completion, Context};
use ets_shim::lsproto;

use crate::*;

// go: internal/completions/effect_jsdoc_comment.go:10 effectJsdocComment ad6c3ac7d8097236
// effectJsdocComment provides completion for @effect-identifier
// when typing @ inside a comment.
pub(crate) static EFFECT_JSDOC_COMMENT: LazyLock<Completion> = LazyLock::new(|| Completion {
    name: "effectJsdocComment",
    description: "Provides @effect-identifier completion inside comments",
    run: run_effect_jsdoc_comment,
});

// go: internal/completions/effect_jsdoc_comment.go:16 runEffectJsdocComment c6c9690446853133
fn run_effect_jsdoc_comment(ctx: &mut Context<'_, '_>) -> Vec<lsproto::CompletionItem> {
    let source_text = ctx.source_file.text();
    let text_before_cursor = &source_text.as_bytes()[..ctx.position as usize];

    let Some(loc) = COMMENT_AT_REGEX.captures(text_before_cursor) else {
        return Vec::new();
    };

    // loc[4] and loc[5] are the start/end byte offsets of the `@` capture group
    let at_start = loc.get(2).unwrap().start() as u32;
    let span_length = (ctx.position as i64 - at_start as i64).max(0) as u32;

    let replacement_range = byte_span_to_range(ctx, at_start, span_length);

    let sort_text = "11";

    vec![make_completion_item(
        "@effect-identifier",
        "@effect-identifier",
        sort_text,
        replacement_range,
        Some(lsproto::CompletionItemKind::CONSTANT),
    )]
}
