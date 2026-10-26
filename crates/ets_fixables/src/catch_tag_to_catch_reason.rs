use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{PipingFlowTransformationReplacement, Tracker};
use ets_rules::{CatchTagToCatchReasonBranch, CatchTagToCatchReasonMatch};
use ets_shim::P;
use ets_shim::ast::{Kind, Node, NodeList, SourceFile, TokenFlags};
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::BRANCHING_ON_0_REASON_TAG_INSIDE_EFFECT_1_HAND_ROLLS_REASON_DISPATCH_USE_EFFECT_CATCH_REASON_OR_EFFECT_CATCH_REASONS_WHICH_RE_FAIL_UNMATCHED_REASONS_AUTOMATICALLY_EFFECT_CATCH_TAG_TO_CATCH_REASON;

// go: internal/fixables/catch_tag_to_catch_reason.go:12 CatchTagToCatchReasonFix
pub static CATCH_TAG_TO_CATCH_REASON_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "catchTagToCatchReason",
    description: "Replace hand-rolled reason dispatch with Effect.catchReason or Effect.catchReasons",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["catchTagToCatchReason_fix"],
    run: run_catch_tag_to_catch_reason_fix,
});

// go: internal/fixables/catch_tag_to_catch_reason.go:22 runCatchTagToCatchReasonFix
//
// Rust: the match's transformation and callee are never nil (non-optional fields), so Go's nil check drops.
fn run_catch_tag_to_catch_reason_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    let matches =
        ets_rules::analyze_catch_tag_to_catch_reason(&mut ctx.type_parser, Some(source_file));
    for m in matches {
        if !m.can_fix || (!m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location)) {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.catchReason or Effect.catchReasons".to_string(),
            run: Box::new(move |tracker| {
                let (callee, arguments) =
                    build_catch_tag_to_catch_reason_replacement(tracker, source_file, &m);
                if callee.is_none() || arguments.is_none() {
                    return;
                }
                tracker.replace_piping_flow_transformation(
                    Some(source_file),
                    Some(&m.transformation),
                    PipingFlowTransformationReplacement { callee, type_arguments: None, arguments },
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}

// go: internal/fixables/catch_tag_to_catch_reason.go:52 buildCatchTagToCatchReasonReplacement
//
// Rust: the tracker and the match's callee cannot be nil.
fn build_catch_tag_to_catch_reason_replacement(
    tracker: &Tracker,
    sf: P<SourceFile>,
    m: &CatchTagToCatchReasonMatch,
) -> (Option<P<Node>>, Option<P<NodeList>>) {
    let Some(outer_tag) = m.outer_tag else {
        return (None, None);
    };
    if m.branches.is_empty() {
        return (None, None);
    }
    let mut receiver = None;
    if m.callee.kind() == Kind::PropertyAccessExpression {
        receiver = Some(m.callee.as_property_access_expression().expression());
    }

    let mut method_name = "catchReason";
    if m.branches.len() > 1 {
        method_name = "catchReasons";
    }
    let method = effect_module_method(tracker, sf, receiver, method_name);
    let mut arguments = vec![tracker.deep_clone_node(Some(outer_tag)).unwrap()];
    if m.branches.len() == 1 {
        let branch = &m.branches[0];
        arguments.push(tracker.new_string_literal(alloc_str(&branch.reason_tag), TokenFlags::None));
        arguments.push(new_catch_reason_handler(tracker, &m.parameter_name, branch));
    } else {
        let mut properties = Vec::with_capacity(m.branches.len());
        for branch in &m.branches {
            properties.push(tracker.new_property_assignment(
                None,
                tracker.new_string_literal(alloc_str(&branch.reason_tag), TokenFlags::None),
                None,
                None,
                new_catch_reason_handler(tracker, &m.parameter_name, branch),
            ));
        }
        arguments
            .push(tracker.new_object_literal_expression(tracker.new_node_list(properties), false));
    }

    (Some(method), Some(tracker.new_node_list(arguments)))
}

// go: internal/fixables/catch_tag_to_catch_reason.go:87 newCatchReasonHandler
fn new_catch_reason_handler(
    tracker: &Tracker,
    parameter_name: &str,
    branch: &CatchTagToCatchReasonBranch,
) -> P<Node> {
    let mut parameters = Vec::new();
    if branch.uses_parameter {
        parameters = vec![
            tracker.new_parameter_declaration(
                None,
                None,
                tracker.new_identifier(alloc_str(&branch.reason_parameter_name)),
                None,
                None,
                None,
            ),
            tracker.new_parameter_declaration(
                None,
                None,
                tracker.new_identifier(alloc_str(parameter_name)),
                None,
                None,
                None,
            ),
        ];
    }
    tracker.new_arrow_function(
        None,
        None,
        Some(tracker.new_node_list(parameters)),
        None,
        None,
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        tracker.deep_clone_node(branch.result),
    )
}
