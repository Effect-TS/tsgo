use std::sync::LazyLock;

use ets_completion::{Completion, Context};
use ets_shim::lsproto;

use crate::*;

// go: internal/completions/effect_codegens_comment.go:15 effectCodegensComment
// effectCodegensComment provides completions for @effect-codegens directives
// when typing @ inside a comment.
pub(crate) static EFFECT_CODEGENS_COMMENT: LazyLock<Completion> = LazyLock::new(|| Completion {
    name: "effectCodegensComment",
    description: "Provides @effect-codegens completion inside comments",
    run: run_effect_codegens_comment,
});

// go: internal/completions/effect_codegens_comment.go:21 runEffectCodegensComment
fn run_effect_codegens_comment(ctx: &mut Context<'_, '_>) -> Vec<lsproto::CompletionItem> {
    let source_text = ctx.source_file.text();
    let text_before_cursor = &source_text.as_bytes()[..ctx.position as usize];

    let Some(loc) = COMMENT_AT_REGEX.captures(text_before_cursor) else {
        return Vec::new();
    };

    // loc[4] and loc[5] are the start/end byte offsets of the `@` capture group
    let at_start = loc.get(2).unwrap().start() as u32;
    let span_length = (ctx.position as i64 - at_start as i64).max(0) as u32;

    let replacement_range = byte_span_to_range(ctx, at_start, span_length);

    // Collect and sort all codegen names
    // (Rust: Go's `sort.Strings`; the names are unique, so no tie order can show.)
    let mut names: Vec<&str> = ets_codegens::ALL.iter().map(|c| c.name).collect();
    names.sort_unstable();

    let all_codegens = names.join(",");
    let enable_snippet = format!("${{1|{all_codegens}|}} $0");

    let sort_text = "11";

    vec![make_completion_item(
        "@effect-codegens",
        &format!("@effect-codegens {enable_snippet}"),
        sort_text,
        replacement_range,
        Some(lsproto::CompletionItemKind::Constant),
    )]
}
