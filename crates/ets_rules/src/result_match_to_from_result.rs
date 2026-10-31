use std::cell::Cell;
use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, LazyExpressionFlags, PipingFlow, PipingFlowSubject,
    PipingFlowTransformation, TransformationKind, TypeParser,
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

// go: internal/rules/result_match_to_from_result.go:54 AnalyzeResultMatchToFromResult c84786d50bb0b79b
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

// go: internal/rules/result_match_to_from_result.go:75 analyzeResultMatchStep 169b68d2c9e20211
// analyzeResultMatchStep matches a Result.match step whose handlers only lift
// the failure with Effect.fail and the success with Effect.succeed.
//
// Rust: Go's `(match, ok)` is an `Option`; the checker is the parser's. A transformation's callee is never nil, so
// Go's nil check drops.
pub(crate) fn analyze_result_match_step(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    transformation: &PipingFlowTransformation,
) -> Option<ResultMatchToFromResultMatch> {
    match transformation.kind() {
        TransformationKind::Pipe
        | TransformationKind::Pipeable
        | TransformationKind::DataFirst
        | TransformationKind::DataLast
        | TransformationKind::Call => {}
        _ => return None,
    }

    let callee = transformation.callee();
    if transformation.args().len() != 1 || !has_no_type_arguments(transformation) {
        return None;
    }
    let (on_failure, on_success) = result_match_handlers(Some(*transformation.args().at(0)))?;
    if !tp.is_node_reference_to_effect_result_module_api(callee, "match")
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

// go: internal/rules/result_match_to_from_result.go:105 resultMatchHandlers 855832a8b0c4cc79
//
// Rust: Go's `(onFailure, onSuccess, ok)` is an `Option` of the first two.
pub(crate) fn result_match_handlers(node: Option<P<Node>>) -> Option<(P<Node>, P<Node>)> {
    let node = ast::skip_parentheses(node?);
    if node.kind() != Kind::ObjectLiteralExpression {
        return None;
    }
    let object = node.as_object_literal_expression();
    if object.properties.nodes().len() != 2 {
        return None;
    }

    let mut on_failure: Option<P<Node>> = None;
    let mut on_success: Option<P<Node>> = None;
    for &property_node in object.properties.nodes() {
        if property_node.kind() != Kind::PropertyAssignment {
            return None;
        }
        let name = property_node.name()?;
        if name.kind() != Kind::Identifier {
            return None;
        }
        let initializer = property_node.initializer()?;
        match ets_shim::scanner::get_text_of_node(name).as_str() {
            "onFailure" => {
                if on_failure.is_some() {
                    return None;
                }
                on_failure = Some(initializer);
            }
            "onSuccess" => {
                if on_success.is_some() {
                    return None;
                }
                on_success = Some(initializer);
            }
            _ => return None,
        }
    }

    Some((on_failure?, on_success?))
}

// go: internal/rules/result_match_to_from_result.go:142 hasNoTypeArguments 1f39635fff140d67
pub(crate) fn has_no_type_arguments(transformation: &PipingFlowTransformation) -> bool {
    transformation.type_arguments().is_none_or(|arguments| arguments.nodes().is_empty())
}

// go: internal/rules/result_match_to_from_result.go:148 isEffectLiftStep 331913a366849bad
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

// go: internal/rules/result_match_to_from_result.go:158 isEffectLiftHandler cb9f7bbbb858d191
// isEffectLiftHandler reports whether a Result.match handler passes its
// argument unchanged to Effect.<api>: either the API itself, or a function of
// one parameter whose returned flow only applies Effect.<api> to it.
//
// Rust: the checker is the parser's; Go's nil checks on the parameter name and the lazy expression drop, both are
// non-optional here.
pub(crate) fn is_effect_lift_handler(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
    api: &'static str,
) -> bool {
    let node = ast::skip_parentheses(node);
    if tp.is_node_reference_to_effect_module_api(node, api) {
        return true;
    }
    let Some(lazy) = ets_typeparser::parse_lazy_expression(node, LazyExpressionFlags::None) else {
        return false;
    };
    if lazy.params().len() != 1 || lazy.params().at(0).kind() != Kind::Parameter {
        return false;
    }
    let parameter = lazy.params().at(0).as_parameter_declaration();
    let parameter_name = parameter.name();
    if parameter_name.kind() != Kind::Identifier
        || parameter.dot_dot_dot_token().is_some()
        || parameter.initializer().is_some()
    {
        return false;
    }
    let flow = tp.longest_piping_flow_at(lazy.expression(), false);
    let is_parameter = |tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| -> bool {
        same_identifier_reference(
            tp,
            Some(parameter_name),
            subject.node().map(ast::skip_parentheses),
        )
    };
    PipingFlow::matches_exactly(flow.as_ref(), tp, &is_parameter, &[&is_effect_lift_step(api)])
}

// go: internal/rules/result_match_to_from_result.go:182 analyzeResultGuardConditional d8e19988545e31a6
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
        if transformation.args().len() != 0 || !has_no_type_arguments(transformation) {
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
    let mut condition = flow.node();
    while let Some(parent) = condition.parent()
        && parent.kind() == Kind::ParenthesizedExpression
    {
        condition = parent;
    }
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

// go: internal/rules/result_match_to_from_result.go:232 isResultMemberLift 391e2f4c0b3de763
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
