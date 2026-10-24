use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{PipingFlowTransformationReplacement, Tracker};
use ets_rules::FlatMapConditionalToFilterOrFailMatch;
use ets_shim::P;
use ets_shim::ast::{Kind, Node, NodeList, SourceFile};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::Effect_0_expresses_this_conditional_validation_more_directly_than_Effect_flatMap_with_an_identity_Effect_succeed_branch_effect_flatMapConditionalToFilterOrFail;

// go: internal/fixables/flat_map_conditional_to_filter_or_fail.go:12 FlatMapConditionalToFilterOrFailFix
pub static FLAT_MAP_CONDITIONAL_TO_FILTER_OR_FAIL_FIX: LazyLock<Fixable> = LazyLock::new(|| {
    Fixable {
        name: "flatMapConditionalToFilterOrFail",
        description: "Replace a conditional Effect.flatMap identity filter with Effect.filterOrFail or Effect.filterOrElse",
        error_codes: vec![MESSAGE.code()],
        fix_ids: vec!["flatMapConditionalToFilterOrFail_fix"],
        run: run_flat_map_conditional_to_filter_or_fail_fix,
    }
});

// go: internal/fixables/flat_map_conditional_to_filter_or_fail.go:22 runFlatMapConditionalToFilterOrFailFix
//
// Rust: the match's transformation, parameter, predicate and fallback are never nil (non-optional fields), so only
// Go's `CanFix` check remains.
fn run_flat_map_conditional_to_filter_or_fail_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in
        ets_rules::analyze_flat_map_conditional_to_filter_or_fail(&mut ctx.type_parser, source_file)
    {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if !m.can_fix {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: format!("Replace with Effect.{}", m.preferred_method_name),
            run: Box::new(move |tracker| {
                let (callee, arguments) =
                    build_flat_map_conditional_filter_replacement(tracker, source_file, &m);
                tracker.replace_piping_flow_transformation(
                    Some(source_file),
                    Some(&m.transformation),
                    PipingFlowTransformationReplacement {
                        callee: Some(callee),
                        type_arguments: None,
                        arguments: Some(arguments),
                    },
                );
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/flat_map_conditional_to_filter_or_fail.go:55 buildFlatMapConditionalFilterReplacement
//
// Rust: both results are always non-nil in Go too, so they are returned unwrapped (the caller's nil check drops).
fn build_flat_map_conditional_filter_replacement(
    tracker: &Tracker,
    sf: P<SourceFile>,
    m: &FlatMapConditionalToFilterOrFailMatch,
) -> (P<Node>, P<NodeList>) {
    let callee = effect_module_method(tracker, sf, m.effect_module_node, m.preferred_method_name);
    let mut predicate_body = tracker.deep_clone_node(Some(m.predicate_node)).unwrap();
    if m.negate_predicate {
        predicate_body =
            tracker.new_prefix_unary_expression(Kind::ExclamationToken, predicate_body);
    }
    let parameter = tracker.deep_clone_node(Some(m.parameter_node)).unwrap();
    let predicate = tracker.new_arrow_function(
        None,
        None,
        Some(tracker.new_node_list(vec![parameter])),
        None,
        None,
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        Some(predicate_body),
    );
    let fallback = tracker.new_arrow_function(
        None,
        None,
        Some(tracker.new_node_list(vec![tracker.deep_clone_node(Some(m.parameter_node)).unwrap()])),
        None,
        None,
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        tracker.deep_clone_node(Some(m.fallback_node)),
    );
    (callee, tracker.new_node_list(vec![predicate, fallback]))
}
