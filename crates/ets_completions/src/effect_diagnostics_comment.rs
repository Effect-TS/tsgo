use std::sync::LazyLock;

use ets_completion::{Completion, Context};
use ets_shim::lsproto;
use regex::bytes::Regex;

use crate::*;

// go: internal/completions/effect_diagnostics_comment.go:16 effectDiagnosticsComment 662fdcc24ce099b0
// effectDiagnosticsComment provides completions for @effect-diagnostics and
// @effect-diagnostics-next-line directives when typing @ inside a comment.
pub(crate) static EFFECT_DIAGNOSTICS_COMMENT: LazyLock<Completion> = LazyLock::new(|| Completion {
    name: "effectDiagnosticsComment",
    description: "Provides @effect-diagnostics completion inside comments",
    run: run_effect_diagnostics_comment,
});

// go: internal/completions/effect_diagnostics_comment.go:23 commentAtRegex 7f18371138f86adf
// commentAtRegex matches `//`, `/*`, or `/**` followed by optional whitespace and `@` at end of string.
//
// Rust: RE2's `\s` is `[\t\n\f\r ]` (porting/PORTING.md, "Types"). A byte regex, as Go's matches the bytes of a
// string sliced at any byte offset.
pub(crate) static COMMENT_AT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(//|/\*\*?)[\t\n\f\r ]*(@)[\t\n\f\r ]*$").unwrap());

// go: internal/completions/effect_diagnostics_comment.go:25 runEffectDiagnosticsComment a7faf421ca45a56c
fn run_effect_diagnostics_comment(ctx: &mut Context<'_, '_>) -> Vec<lsproto::CompletionItem> {
    let source_text = ctx.source_file.text();
    let text_before_cursor = &source_text.as_bytes()[..ctx.position as usize];

    let Some(loc) = COMMENT_AT_REGEX.captures(text_before_cursor) else {
        return Vec::new();
    };

    // loc[4] and loc[5] are the start/end byte offsets of the `@` capture group
    let at_start = loc.get(2).unwrap().start() as u32;
    let span_length = (ctx.position as i64 - at_start as i64).max(0) as u32;

    let replacement_range = byte_span_to_range(ctx, at_start, span_length);

    // Collect and sort all diagnostic rule names
    // (Rust: Go's `sort.Strings`; the names are unique, so no tie order can show.)
    let mut names: Vec<&str> = ets_rules::ALL.iter().map(|r| r.name).collect();
    names.sort_unstable();

    let all_diagnostics = names.join(",");
    let disable_snippet =
        format!("${{1|{all_diagnostics}|}}:${{2|off,warning,error,message,suggestion|}}$0");

    let sort_text = "11";

    vec![
        make_completion_item(
            "@effect-diagnostics",
            &format!("@effect-diagnostics {disable_snippet}"),
            sort_text,
            replacement_range,
            Some(lsproto::CompletionItemKind::CONSTANT),
        ),
        make_completion_item(
            "@effect-diagnostics-next-line",
            &format!("@effect-diagnostics-next-line {disable_snippet}"),
            sort_text,
            replacement_range,
            Some(lsproto::CompletionItemKind::CONSTANT),
        ),
    ]
}
