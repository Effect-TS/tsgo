use std::sync::LazyLock;

use ets_readonly::Slice;
use ets_refactor::{Context, Refactor, RefactorAction};
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::astnav;
use ets_shim::core::P;
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;
use ets_shim::scanner;
use ets_typeparser::TransformationKind;

use crate::*;

// go: internal/refactors/toggle_pipe_style.go:17 TogglePipeStyle
pub static TOGGLE_PIPE_STYLE: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "togglePipeStyle",
    description: "Toggle pipe style",
    kind: "rewrite.effect.togglePipeStyle",
    run: run_toggle_pipe_style,
});

// go: internal/refactors/toggle_pipe_style.go:24 runTogglePipeStyle
pub(crate) fn run_toggle_pipe_style(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    // Walk up the ancestor chain looking for a pipe call
    for node in std::iter::successors(Some(token), |n| n.parent()) {
        if node.kind() != Kind::CallExpression {
            continue;
        }

        let Some(pipe_call) = ctx.type_parser.parse_pipe_call(node) else {
            continue;
        };

        match pipe_call.kind() {
            TransformationKind::Pipe => {
                // pipe(subject, f1, f2) -> subject.pipe(f1, f2)
                // Check that the subject's type is pipeable
                let subject_type = ctx.type_parser.get_type_at_location(pipe_call.subject());
                if !ctx.type_parser.is_pipeable_type(subject_type) {
                    continue;
                }

                let sf = ctx.source_file;
                let ctx_ref = &*ctx;
                let Some(mut action) = ctx_ref.new_refactor_action(RefactorAction {
                    description: "Rewrite as X.pipe(Y, Z, ...)".to_string(),
                    run: Box::new(|tracker| {
                        let start = astnav::get_start_of_node(node, sf, false);
                        let rewritten =
                            toggle_pipe_to_method_text(sf, pipe_call.subject(), pipe_call.args());
                        tracker.replace_range_with_text(
                            sf,
                            lsproto::Range {
                                start: ctx_ref.byte_pos_to_lsp_position(start),
                                end: ctx_ref.byte_pos_to_lsp_position(
                                    node.end().expect_source("a parsed node"),
                                ),
                            },
                            &rewritten,
                        );
                    }),
                }) else {
                    return Vec::new();
                };
                action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.togglePipeStyle");
                return vec![action];
            }

            TransformationKind::Pipeable => {
                // subject.pipe(f1, f2) -> pipe(subject, f1, f2)
                let sf = ctx.source_file;
                let ctx_ref = &*ctx;
                let Some(mut action) = ctx_ref.new_refactor_action(RefactorAction {
                    description: "Rewrite as pipe(X, Y, Z, ...)".to_string(),
                    run: Box::new(|tracker| {
                        let start = astnav::get_start_of_node(node, sf, false);
                        let rewritten =
                            toggle_pipe_to_function_text(sf, pipe_call.subject(), pipe_call.args());
                        tracker.replace_range_with_text(
                            sf,
                            lsproto::Range {
                                start: ctx_ref.byte_pos_to_lsp_position(start),
                                end: ctx_ref.byte_pos_to_lsp_position(
                                    node.end().expect_source("a parsed node"),
                                ),
                            },
                            &rewritten,
                        );
                    }),
                }) else {
                    return Vec::new();
                };
                action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.togglePipeStyle");
                return vec![action];
            }

            _ => {}
        }
    }

    Vec::new()
}

// go: internal/refactors/toggle_pipe_style.go:91 togglePipeToMethodText
pub(crate) fn toggle_pipe_to_method_text(
    sf: P<SourceFile>,
    subject: P<Node>,
    args: Slice<'_, P<Node>>,
) -> String {
    let mut parts = Vec::with_capacity(usize::try_from(args.len()).unwrap_or_default());
    for (_, &arg) in args.all() {
        parts.push(scanner::get_source_text_of_node_from_source_file(sf, arg, false));
    }
    scanner::get_source_text_of_node_from_source_file(sf, subject, false)
        + ".pipe("
        + &parts.join(", ")
        + ")"
}

// go: internal/refactors/toggle_pipe_style.go:99 togglePipeToFunctionText
pub(crate) fn toggle_pipe_to_function_text(
    sf: P<SourceFile>,
    subject: P<Node>,
    args: Slice<'_, P<Node>>,
) -> String {
    let mut parts = Vec::with_capacity(usize::try_from(args.len()).unwrap_or_default() + 1);
    parts.push(scanner::get_source_text_of_node_from_source_file(sf, subject, false));
    for (_, &arg) in args.all() {
        parts.push(scanner::get_source_text_of_node_from_source_file(sf, arg, false));
    }
    "pipe(".to_string() + &parts.join(", ") + ")"
}
