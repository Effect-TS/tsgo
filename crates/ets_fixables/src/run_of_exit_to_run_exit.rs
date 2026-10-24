use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::Tracker;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::core::{P, TextRange};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::scanner;
use ets_typeparser::{PipingFlowTransformation, TransformationKind};

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::Effect_0_of_Effect_exit_re_implements_Effect_1_Use_the_dedicated_Exit_runner_directly_effect_runOfExitToRunExit;

// go: internal/fixables/run_of_exit_to_run_exit.go:15 RunOfExitToRunExitFix
pub static RUN_OF_EXIT_TO_RUN_EXIT_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "runOfExitToRunExit",
    description: "Replace with the dedicated Effect Exit runner",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["runOfExitToRunExit_fix"],
    run: run_run_of_exit_to_run_exit_fix,
});

// go: internal/fixables/run_of_exit_to_run_exit.go:25 runRunOfExitToRunExitFix
//
// Rust: the match's exit transformation is never nil (an owned copy), so only the callee is checked.
fn run_run_of_exit_to_run_exit_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_run_of_exit_to_run_exit(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let Some(runner_callee) = m.runner_callee else {
            continue;
        };

        let description = "Replace with Effect.".to_string() + m.replacement_name;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                if !remove_exit_transformation(tracker, Some(sf), Some(&m.exit_transformation)) {
                    return;
                }
                replace_effect_method_callee(
                    tracker,
                    sf,
                    Some(runner_callee),
                    m.runner_name_node,
                    m.replacement_name,
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}

// go: internal/fixables/run_of_exit_to_run_exit.go:52 removeExitTransformation
//
// Rust: Go's nil tracker check has no counterpart (`&mut Tracker`); a transformation's callee is never nil.
// tsrs's call arguments are never nil, so those checks drop.
pub(crate) fn remove_exit_transformation(
    tracker: &mut Tracker,
    sf: Option<P<SourceFile>>,
    transformation: Option<&PipingFlowTransformation>,
) -> bool {
    let (Some(sf), Some(transformation)) = (sf, transformation) else {
        return false;
    };

    match transformation.kind() {
        TransformationKind::DataFirst | TransformationKind::DataLast | TransformationKind::Call => {
            let Some(call_node) = direct_call_for_callee(Some(transformation.callee())) else {
                return false;
            };
            let call = call_node.as_call_expression();
            if call.arguments().nodes().len() != 1 {
                return false;
            }
            let replacement = tracker.deep_clone_node(Some(call.arguments().nodes()[0]));
            tracker.replace_node(sf, Some(call_node), replacement, None);
            true
        }

        TransformationKind::Pipe | TransformationKind::Pipeable => {
            let (Some(call_node), argument_index) =
                containing_call_argument_for_exit(Some(transformation.callee()))
            else {
                return false;
            };
            if argument_index < 0 {
                return false;
            }
            let call = call_node.as_call_expression();
            let arguments = call.arguments().nodes();
            let mut minimum_arguments = 1;
            if transformation.kind() == TransformationKind::Pipe {
                minimum_arguments = 2; // Function.pipe also contains its subject.
            }
            if arguments.len() == minimum_arguments {
                let mut subject = None;
                if transformation.kind() == TransformationKind::Pipe {
                    subject = Some(arguments[0]);
                } else if call.expression().kind() == Kind::PropertyAccessExpression {
                    subject = Some(call.expression().as_property_access_expression().expression());
                }
                let Some(subject) = subject else {
                    return false;
                };
                let replacement = tracker.deep_clone_node(Some(subject));
                tracker.replace_node(sf, Some(call_node), replacement, None);
                return true;
            }

            let argument_index = argument_index as usize;
            if argument_index < arguments.len() - 1 {
                let start = scanner::get_token_pos_of_node(arguments[argument_index], sf, false);
                let end = scanner::get_token_pos_of_node(arguments[argument_index + 1], sf, false);
                tracker.delete_range(sf, TextRange::new(start, end));
                return true;
            }
            if argument_index > 0 {
                tracker.delete_range(
                    sf,
                    TextRange::from_text_pos(
                        arguments[argument_index - 1].end(),
                        arguments[argument_index].end(),
                    ),
                );
                return true;
            }
            false
        }

        _ => false,
    }
}

// go: internal/fixables/run_of_exit_to_run_exit.go:105 directCallForCallee
//
// Rust: returns the call node (Go: the node and its `*ast.CallExpression`, read with `as_call_expression()`).
pub(crate) fn direct_call_for_callee(callee: Option<P<Node>>) -> Option<P<Node>> {
    let callee = callee?;
    let parent = callee.parent()?;
    if parent.kind() != Kind::CallExpression {
        return None;
    }
    let call = parent.as_call_expression();
    if call.expression() != callee {
        return None;
    }
    Some(parent)
}

// go: internal/fixables/run_of_exit_to_run_exit.go:116 containingCallArgumentForExit
//
// Rust: returns the call node and the argument index (Go also returns the node's `*ast.CallExpression`).
pub(crate) fn containing_call_argument_for_exit(node: Option<P<Node>>) -> (Option<P<Node>>, i64) {
    let mut child = node;
    while let Some(c) = child {
        let Some(parent) = c.parent() else {
            break;
        };
        if parent.kind() == Kind::CallExpression {
            let call = parent.as_call_expression();
            for (index, &argument) in call.arguments().nodes().iter().enumerate() {
                if argument == c {
                    return (Some(parent), index as i64);
                }
            }
        }
        child = Some(parent);
    }
    (None, -1)
}
