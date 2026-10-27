use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

use crate::*;

// go: internal/refactors/wrap_with_pipe.go:9 WrapWithPipe 673f9ab5c746823f
pub static WRAP_WITH_PIPE: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "wrapWithPipe",
    description: "Wrap with pipe",
    kind: "rewrite.effect.wrapWithPipe",
    run: run_wrap_with_pipe,
});

// go: internal/refactors/wrap_with_pipe.go:16 runWrapWithPipe 192e8033c3d20508
pub(crate) fn run_wrap_with_pipe(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    // Only applicable when the selection is non-empty.
    if ctx.span.pos() == ctx.span.end() {
        return Vec::new();
    }

    let start_pos = ctx.byte_pos_to_lsp_position(span_pos(ctx));
    let end_pos = ctx.byte_pos_to_lsp_position(ctx.span.end().expect_source("a request span"));

    let source_file = ctx.source_file;
    let Some(mut action) = ctx.new_refactor_action(RefactorAction {
        description: "Wrap with pipe".to_string(),
        run: Box::new(move |tracker| {
            tracker.insert_text(source_file, start_pos, "pipe(");
            tracker.insert_text(source_file, end_pos, ")");
        }),
    }) else {
        return Vec::new();
    };

    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.wrapWithPipe");
    vec![action]
}
