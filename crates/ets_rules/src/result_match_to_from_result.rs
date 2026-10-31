use std::cell::Cell;
use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, PipingFlow, PipingFlowSubject, PipingFlowTransformation, TypeParser,
};

use crate::*;

/// Rust-only: Go's message identifier, bound once so rustfmt can lay out the `pub static`.
static MESSAGE: &Message = &tsdiag::EFFECT_FROM_RESULT_EXPRESSES_THIS_RESULT_TO_EFFECT_CONVERSION_MORE_DIRECTLY_THAN_RESULT_MATCH_OR_A_RESULT_GUARD_CONDITIONAL_EFFECT_RESULT_MATCH_TO_FROM_RESULT;

// go: internal/rules/result_match_to_from_result.go:17 ResultMatchToFromResult f4434d3691d573d0
// ResultMatchToFromResult suggests Effect.fromResult when Result.match or a
// Result guard conditional only lifts Success with Effect.succeed and Failure
// with Effect.fail.
pub static RESULT_MATCH_TO_FROM_RESULT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "resultMatchToFromResult",
    group: "style",
    description: "Suggests Effect.fromResult when Result.match or a Result guard conditional only converts Success to Effect.succeed and Failure to Effect.fail",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let matches = analyze_result_match_to_from_result(ctx.type_parser, Some(ctx.source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/result_match_to_from_result.go:44 ResultMatchToFromResultMatch f4177ea90eff5084
// ResultMatchToFromResultMatch holds the nodes needed by the diagnostic and
// quick fix. Result.match steps set Transformation; guard conditionals set
// ReplacementNode and ResultNode.
//
// Rust: Go's `*typeparser.PipingFlowTransformation` points at a copy of the flow's transformation; the match owns
// that copy.
#[derive(Clone)]
pub struct ResultMatchToFromResultMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub transformation: Option<PipingFlowTransformation>,
    pub replacement_node: Option<P<Node>>,
    pub result_node: Option<P<Node>>,
}

// go: internal/rules/result_match_to_from_result.go:54 AnalyzeResultMatchToFromResult 97a2f78cb93fc471
// AnalyzeResultMatchToFromResult finds Result.match steps and Result guard
// conditionals that are equivalent to Effect.fromResult.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), so it is dropped.
pub fn analyze_result_match_to_from_result(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<ResultMatchToFromResultMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        for (index, _) in flow.transformations().all() {
            if let Some(m) = analyze_result_match_step(tp, sf, flow.transformations().at(index)) {
                matches.push(m);
            }
        }
        if let Some(m) = analyze_result_guard_conditional(tp, sf, flow) {
            matches.push(m);
        }
    }
    matches
}

// go: internal/rules/result_match_to_from_result.go:75 analyzeResultMatchStep 4d5ab7675cadacb0
// analyzeResultMatchStep matches a Result.match step whose handlers only lift
// the failure with Effect.fail and the success with Effect.succeed.
//
// Rust: Go's `(match, ok)` is an `Option`. A transformation's callee is never nil, so
// Go's nil check drops.
pub(crate) fn analyze_result_match_step(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    transformation: &PipingFlowTransformation,
) -> Option<ResultMatchToFromResultMatch> {
    let callee = transformation.callee();
    if transformation.args().len() != 1 || !has_no_type_arguments(transformation) {
        return None;
    }
    let handlers = ast::skip_parentheses(*transformation.args().at(0));
    let on_failure = ets_typeparser::object_literal_property_initializer(handlers, "onFailure");
    let on_success = ets_typeparser::object_literal_property_initializer(handlers, "onSuccess");
    let (Some(on_failure), Some(on_success)) = (on_failure, on_success) else {
        return None;
    };
    if handlers.as_object_literal_expression().properties.nodes().len() != 2
        || !tp.is_node_reference_to_effect_result_module_api(callee, "match")
        || !is_effect_lift_handler(tp, on_success, "succeed")
        || !is_effect_lift_handler(tp, on_failure, "fail")
    {
        return None;
    }

    Some(ResultMatchToFromResultMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, callee),
        transformation: Some(transformation.clone()),
        replacement_node: None,
        result_node: None,
    })
}

// go: internal/rules/result_match_to_from_result.go:97 hasNoTypeArguments 1f39635fff140d67
pub(crate) fn has_no_type_arguments(transformation: &PipingFlowTransformation) -> bool {
    transformation.type_arguments().is_none_or(|arguments| arguments.nodes().is_empty())
}

// go: internal/rules/result_match_to_from_result.go:103 isEffectLiftStep 331913a366849bad
// isEffectLiftStep reports whether a step applies Effect.<api> to its input
// without extra or type arguments.
//
// Rust: the returned predicate takes the parser as its first argument, like every flow predicate.
pub(crate) fn is_effect_lift_step(
    api: &'static str,
) -> impl Fn(&mut TypeParser<'_>, &PipingFlowTransformation) -> bool {
    move |tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| -> bool {
        transformation.args().len() == 0
            && has_no_type_arguments(transformation)
            && tp.is_node_reference_to_effect_module_api(transformation.callee(), api)
    }
}

// go: internal/rules/result_match_to_from_result.go:112 isEffectLiftHandler 8211651107155f0d
// isEffectLiftHandler reports whether a Result.match handler is Effect.<api>,
// directly or through an identity forwarder, without type arguments.
pub(crate) fn is_effect_lift_handler(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
    api: &'static str,
) -> bool {
    let (target, type_arguments, _) = tp.unwrap_identity_forwarder(node);
    type_arguments.is_none_or(|type_arguments| type_arguments.nodes().is_empty())
        && target.is_some_and(|target| tp.is_node_reference_to_effect_module_api(target, api))
}

// go: internal/rules/result_match_to_from_result.go:121 analyzeResultGuardConditional 8811d0eb3ad87132
// analyzeResultGuardConditional matches a flow that applies Result.isSuccess or
// Result.isFailure to an identifier and is the condition of a conditional
// whose branches lift that identifier's .success with Effect.succeed and its
// .failure with Effect.fail.
//
// Rust: Go's `(match, ok)` is an `Option`; the checker is the parser's. Go's closure assigns `isFailure`, which is a
// `Cell` here because flow predicates are `Fn`.
pub(crate) fn analyze_result_guard_conditional(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    flow: &PipingFlow,
) -> Option<ResultMatchToFromResultMatch> {
    let is_failure = Cell::new(false);
    let is_guard = |tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| -> bool {
        if transformation.args().len() != 0 {
            return false;
        }
        if tp.is_node_reference_to_effect_result_module_api(transformation.callee(), "isSuccess") {
            return true;
        }
        is_failure.set(
            tp.is_node_reference_to_effect_result_module_api(transformation.callee(), "isFailure"),
        );
        is_failure.get()
    };
    let is_identifier = |_: &mut TypeParser<'_>, subject: &PipingFlowSubject| -> bool {
        subject.node().is_some_and(|node| node.kind() == Kind::Identifier)
    };
    if !PipingFlow::matches_exactly(Some(flow), tp, &is_identifier, &[&is_guard]) {
        return None;
    }

    // The conditional is not a call chain, so it is read from the syntax tree.
    let condition = flow.node();
    let parent = condition.parent()?;
    if parent.kind() != Kind::ConditionalExpression
        || parent.as_conditional_expression().condition() != condition
    {
        return None;
    }
    let conditional = parent.as_conditional_expression();

    let result_node = flow.subject().node()?;
    let mut succeed_node = conditional.when_true();
    let mut fail_node = conditional.when_false();
    if is_failure.get() {
        std::mem::swap(&mut succeed_node, &mut fail_node);
    }
    if !is_result_member_lift(tp, result_node, succeed_node, "succeed", "success")
        || !is_result_member_lift(tp, result_node, fail_node, "fail", "failure")
    {
        return None;
    }

    Some(ResultMatchToFromResultMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(
            sf,
            flow.transformations().at(0).callee(),
        ),
        transformation: None,
        replacement_node: Some(parent),
        result_node: Some(result_node),
    })
}

// go: internal/rules/result_match_to_from_result.go:168 isResultMemberLift 391e2f4c0b3de763
// isResultMemberLift reports whether node is a flow that only applies
// Effect.<api> to resultNode.<property>.
//
// Rust: the checker is the parser's.
pub(crate) fn is_result_member_lift(
    tp: &mut TypeParser<'_>,
    result_node: P<Node>,
    node: P<Node>,
    api: &'static str,
    property: &str,
) -> bool {
    let flow = tp.longest_piping_flow_at(node, false);
    let is_member = |tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| -> bool {
        let Some(value) = subject.node().map(ast::skip_parentheses) else {
            return false;
        };
        if value.kind() != Kind::PropertyAccessExpression {
            return false;
        }
        let access = value.as_property_access_expression();
        value.question_dot_token().is_none()
            && ets_shim::scanner::get_text_of_node(access.name) == property
            && same_identifier_reference(
                tp,
                Some(result_node),
                Some(ast::skip_parentheses(access.expression)),
            )
    };
    PipingFlow::matches_exactly(flow.as_ref(), tp, &is_member, &[&is_effect_lift_step(api)])
}
