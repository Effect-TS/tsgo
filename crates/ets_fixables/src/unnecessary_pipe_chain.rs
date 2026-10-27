use std::sync::{Arc, LazyLock};

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, NodeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::TransformationKind;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_EXPRESSION_CONTAINS_CHAINED_PIPE_CALLS_THAT_CAN_BE_SIMPLIFIED_TO_A_SINGLE_PIPE_CALL_EFFECT_UNNECESSARY_PIPE_CHAIN;

// go: internal/fixables/unnecessary_pipe_chain.go:13 UnnecessaryPipeChainFix 45a37c9c27506c4c
pub static UNNECESSARY_PIPE_CHAIN_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "unnecessaryPipeChain",
    description: "Rewrite as single pipe call",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["unnecessaryPipeChain_fix"],
    run: run_unnecessary_pipe_chain_fix,
});

// go: internal/fixables/unnecessary_pipe_chain.go:21 runUnnecessaryPipeChainFix bc6c3f8a7bafe3fe
fn run_unnecessary_pipe_chain_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_unnecessary_pipe_chain(&mut ctx.type_parser, sf);

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

    let outer = Arc::clone(&m.outer);
    let inner = Arc::clone(&m.inner);

    if let Some(action) = ctx.new_fix_action(FixAction {
        description: "Rewrite as single pipe call".to_string(),
        run: Box::new(move |tracker| {
            // Collect all arguments: deep-clone inner args then outer args
            let mut all_args =
                Vec::with_capacity((inner.args().len() + outer.args().len()) as usize);
            for (_, &arg) in inner.args().all() {
                all_args.push(tracker.deep_clone_node(Some(arg)).unwrap());
            }
            for (_, &arg) in outer.args().all() {
                all_args.push(tracker.deep_clone_node(Some(arg)).unwrap());
            }

            let replacement_node = match inner.kind() {
                TransformationKind::Pipe => {
                    // pipe(subject, f1, f2) + pipe(..., f3) => pipe(subject, f1, f2, f3)
                    // Deep-clone the original callee (bare `pipe` or `Function.pipe`)
                    let cloned_callee = tracker
                        .deep_clone_node(Some(inner.node().as_call_expression().expression()))
                        .unwrap();
                    // Build args: subject first, then all pipe args
                    let mut call_args = Vec::with_capacity(1 + all_args.len());
                    call_args.push(tracker.deep_clone_node(Some(inner.subject())).unwrap());
                    call_args.extend(all_args);
                    tracker.new_call_expression(
                        cloned_callee,
                        None,
                        None,
                        tracker.new_node_list(call_args),
                        NodeFlags::None,
                    )
                }
                TransformationKind::Pipeable => {
                    // subject.pipe(f1, f2).pipe(f3) => subject.pipe(f1, f2, f3)
                    let cloned_subject = tracker.deep_clone_node(Some(inner.subject())).unwrap();
                    let pipe_access = tracker.new_property_access_expression(
                        cloned_subject,
                        None,
                        tracker.new_identifier("pipe"),
                        NodeFlags::None,
                    );
                    tracker.new_call_expression(
                        pipe_access,
                        None,
                        None,
                        tracker.new_node_list(all_args),
                        NodeFlags::None,
                    )
                }
                _ => return,
            };

            ast::set_parent_in_children(replacement_node);
            tracker.replace_node(sf, Some(outer.node()), Some(replacement_node), None);
        }),
    }) {
        return vec![action];
    }
    Vec::new()
}
