use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_shim::ast::{self, Kind, Node, NodeFlags};
use ets_shim::astnav;
use ets_shim::checker::SignatureKind;
use ets_shim::core::P;
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;
use ets_typeparser::{ParsedPipeCallResult, TransformationKind};

use crate::*;

// go: internal/refactors/pipeable_to_datafirst.go:13 PipeableToDatafirst 1e96e96e92f4c027
pub static PIPEABLE_TO_DATAFIRST: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "pipeableToDatafirst",
    description: "Rewrite to datafirst",
    kind: "rewrite.effect.pipeableToDatafirst",
    run: run_pipeable_to_datafirst,
});

// go: internal/refactors/pipeable_to_datafirst.go:20 runPipeableToDatafirst 2cdb8f410d1a1121
pub(crate) fn run_pipeable_to_datafirst(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    // Walk up the ancestor chain looking for a pipe(...) call
    for node in std::iter::successors(Some(token), |n| n.parent()) {
        if node.kind() != Kind::CallExpression {
            continue;
        }

        let Some(pipe_call) = ctx.type_parser.parse_pipe_call(node) else {
            continue;
        };

        // Only applies to pipe() calls (not .pipe())
        if pipe_call.kind() != TransformationKind::Pipe {
            continue;
        }

        // The pipe identifier must be within the selection range
        let call_expr = node.as_call_expression();
        let expr_pos = call_expr.expression().pos();
        let expr_end = call_expr.expression().end();
        if expr_pos > ctx.span.end() || expr_end < ctx.span.pos() {
            continue;
        }

        // Need at least one argument after the subject
        if pipe_call.args().len() == 0 {
            continue;
        }

        // Try converting pipe args to data-first style
        let result = try_convert_to_datafirst(&pipe_call, ctx, node);
        if result.is_empty() {
            continue;
        }

        return result;
    }

    Vec::new()
}

// go: internal/refactors/pipeable_to_datafirst.go:72 tryConvertToDatafirst 638659cadf3fbae2
// Rust: Go's `c` is `ctx.checker()`; Go's nil result is an empty `Vec`.
pub(crate) fn try_convert_to_datafirst(
    pipe_call: &ParsedPipeCallResult,
    ctx: &mut Context<'_, '_>,
    node: P<Node>,
) -> Vec<CodeAction> {
    // Accumulate: start with the subject, then process each pipe argument
    // Track whether we successfully converted at least one step
    #[derive(Clone, Copy)]
    struct ConversionStep {
        is_data_first: bool,
        arg: Option<P<Node>>, // original arg node
    }

    let mut steps = vec![
        ConversionStep { is_data_first: false, arg: None };
        usize::try_from(pipe_call.args().len()).unwrap_or_default()
    ];
    let mut did_something = false;

    for (i, &arg) in pipe_call.args().all() {
        let i = usize::try_from(i).unwrap();
        if arg.kind() == Kind::CallExpression {
            let arg_call = arg.as_call_expression();
            let expr_type = ctx.type_parser.get_type_at_location(arg_call.expression());
            if let Some(expr_type) = expr_type {
                let call_sigs =
                    ctx.checker().get_signatures_of_type_exported(expr_type, SignatureKind::Call);
                let arg_count = arg_call.arguments().nodes().len();
                for sig in call_sigs {
                    if sig.parameters().len() == arg_count + 1 {
                        steps[i] = ConversionStep { is_data_first: true, arg: Some(arg) };
                        did_something = true;
                        break;
                    }
                }
                if !steps[i].is_data_first {
                    steps[i] = ConversionStep { is_data_first: false, arg: Some(arg) };
                }
                continue;
            }
        }
        steps[i] = ConversionStep { is_data_first: false, arg: Some(arg) };
    }

    if !did_something {
        return Vec::new();
    }

    let sf = ctx.source_file;
    let subject = pipe_call.subject();
    let Some(mut action) = ctx.new_refactor_action(RefactorAction {
        description: "Rewrite to datafirst".to_string(),
        run: Box::new(move |tracker| {
            let clone = |n: P<Node>| tracker.deep_clone_node(Some(n)).unwrap();

            // Build the replacement node by accumulating from the subject
            let mut new_node = clone(subject);

            for step in &steps {
                let step_arg = step.arg.expect("every step is filled");
                if step.is_data_first {
                    // Convert: callee(args...) + self -> callee(self, args...)
                    let arg_call = step_arg.as_call_expression();
                    let cloned_callee = clone(arg_call.expression());

                    let mut all_args = Vec::with_capacity(1 + arg_call.arguments().nodes().len());
                    all_args.push(new_node);
                    for &a in arg_call.arguments().nodes() {
                        all_args.push(clone(a));
                    }

                    let call_expr = tracker.new_call_expression(
                        cloned_callee,
                        None,
                        None,
                        tracker.new_node_list(all_args),
                        NodeFlags::None,
                    );
                    ast::set_parent_in_children(call_expr);
                    new_node = call_expr;
                } else {
                    // Not data-first: keep in pipe form
                    // If newNode is already a pipe call, extend it
                    if is_pipe_call_node(Some(new_node)) {
                        let pipe_expr_call = new_node.as_call_expression();
                        let mut existing_args = Vec::new();
                        existing_args.extend_from_slice(pipe_expr_call.arguments().nodes());
                        existing_args.push(clone(step_arg));

                        let pipe_id = tracker.new_identifier("pipe");
                        let call_expr = tracker.new_call_expression(
                            pipe_id,
                            None,
                            None,
                            tracker.new_node_list(existing_args),
                            NodeFlags::None,
                        );
                        ast::set_parent_in_children(call_expr);
                        new_node = call_expr;
                    } else {
                        // Wrap in pipe(self, arg)
                        let pipe_id = tracker.new_identifier("pipe");
                        let pipe_args = vec![new_node, clone(step_arg)];
                        let call_expr = tracker.new_call_expression(
                            pipe_id,
                            None,
                            None,
                            tracker.new_node_list(pipe_args),
                            NodeFlags::None,
                        );
                        ast::set_parent_in_children(call_expr);
                        new_node = call_expr;
                    }
                }
            }

            tracker.replace_node(sf, Some(node), Some(new_node), None);
        }),
    }) else {
        return Vec::new();
    };

    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.pipeableToDatafirst");
    vec![action]
}

// go: internal/refactors/pipeable_to_datafirst.go:177 isPipeCallNode 50b7a39c1f74b786
// isPipeCallNode checks if a synthesized node is a pipe(...) call by checking if the
// expression is an identifier named "pipe".
pub(crate) fn is_pipe_call_node(node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if node.kind() != Kind::CallExpression {
        return false;
    }
    let call = node.as_call_expression();
    if call.expression().kind() != Kind::Identifier {
        return false;
    }
    call.expression().as_identifier().text() == "pipe"
}
