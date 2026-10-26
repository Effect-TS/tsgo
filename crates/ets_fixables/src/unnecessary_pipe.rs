use std::sync::{Arc, LazyLock};

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_PIPE_CALL_CONTAINS_NO_ARGUMENTS_EFFECT_UNNECESSARY_PIPE;

// go: internal/fixables/unnecessary_pipe.go:12 UnnecessaryPipeFix
pub static UNNECESSARY_PIPE_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "unnecessaryPipe",
    description: "Remove the pipe call",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["unnecessaryPipe_fix"],
    run: run_unnecessary_pipe_fix,
});

// go: internal/fixables/unnecessary_pipe.go:20 runUnnecessaryPipeFix
fn run_unnecessary_pipe_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_unnecessary_pipe(&mut ctx.type_parser, sf);

    let mut found = None;
    for m in &matches {
        let diag_range = m.location;
        if diag_range.intersects(ctx.span) || ctx.span.contained_by(diag_range) {
            found = Some(m);
            break;
        }
    }
    let Some(m) = found else {
        return Vec::new();
    };

    let result = Arc::clone(&m.result);
    let call_node = result.node();

    // Unwrap "pipe(subject)" or "subject.pipe()" to just "subject" by deleting the prefix and suffix around the subject.
    if let Some(action) = ctx.new_fix_action(FixAction {
        description: "Remove the pipe call".to_string(),
        run: Box::new(move |tracker| {
            tracker.delete_range(
                sf,
                TextRange::from_text_pos(call_node.pos(), result.subject().pos()),
            );
            tracker.delete_range(
                sf,
                TextRange::from_text_pos(result.subject().end(), call_node.end()),
            );
        }),
    }) {
        return vec![action];
    }
    Vec::new()
}
