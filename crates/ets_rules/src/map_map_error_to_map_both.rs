use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{PipingFlow, PipingFlowTransformation, TransformationKind, TypeParser};

use crate::piping_flow::{has_explicit_type_arguments, is_piping_flow_argument_kind};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_0_EFFECT_1_TRANSFORM_THE_SUCCESS_AND_FAILURE_CHANNELS_ONE_STEP_AT_A_TIME_EFFECT_2_EXPRESSES_BOTH_TRANSFORMATIONS_IN_A_SINGLE_STEP_EFFECT_MAP_MAP_ERROR_TO_MAP_BOTH;

// go: internal/rules/map_map_error_to_map_both.go:18 MapMapErrorToMapBoth ae1a13250929ac22
// MapMapErrorToMapBoth suggests Effect.mapBoth when Effect.map and
// Effect.mapError are adjacent steps of the same piping flow.
pub static MAP_MAP_ERROR_TO_MAP_BOTH: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "mapMapErrorToMapBoth",
    group: "style",
    description: "Suggests Effect.mapBoth when Effect.map and Effect.mapError are adjacent steps of a piping flow",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_map_map_error_to_map_both(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.first_name, m.second_name, m.replacement_name],
            ));
        }
        diagnostics
    },
});

// go: internal/rules/map_map_error_to_map_both.go:39 MapMapErrorToMapBothMatch f2c323b78e9c414d
#[derive(Clone)]
pub struct MapMapErrorToMapBothMatch {
    pub source_file: P<SourceFile>,
    pub flow: PipingFlow,
    pub start: i64,
    pub location: TextRange,
    pub first_name: &'static str,
    pub second_name: &'static str,
    pub replacement_name: &'static str,
    // FirstFailure is true when the first step transforms the failure channel.
    pub first_failure: bool,
    // First and Second are the two steps in flow order.
    pub first: PipingFlowTransformation,
    pub second: PipingFlowTransformation,
    pub fixable: bool,
}

// go: internal/rules/map_map_error_to_map_both.go:53 mapMapErrorStep b9eff7a0d0d73dbd
#[derive(Clone, Copy)]
pub(crate) struct MapMapErrorStep {
    name: &'static str,
    failure: bool,
    eager: bool,
}

// go: internal/rules/map_map_error_to_map_both.go:59 mapMapErrorSteps 7450978eb2dbee50
static MAP_MAP_ERROR_STEPS: [MapMapErrorStep; 4] = [
    MapMapErrorStep { name: "map", failure: false, eager: false },
    MapMapErrorStep { name: "mapError", failure: true, eager: false },
    MapMapErrorStep { name: "mapEager", failure: false, eager: true },
    MapMapErrorStep { name: "mapErrorEager", failure: true, eager: true },
];

// go: internal/rules/map_map_error_to_map_both.go:68 AnalyzeMapMapErrorToMapBoth 7fcfee9bd4ed42eb
// AnalyzeMapMapErrorToMapBoth finds adjacent success and failure mappings in
// piping flows. Pairs are taken left to right without overlap.
//
// Rust: Go's checker argument is reached through the parser and dropped.
pub fn analyze_map_map_error_to_map_both(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<MapMapErrorToMapBothMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    let is_candidate = |tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| -> bool {
        classify_map_map_error_step(tp, step).is_some()
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        let mut next = 0;
        let sequences = PipingFlow::find_transformation_sequences(
            Some(flow),
            tp,
            &[&is_candidate, &is_candidate],
        );
        for (_, sequence) in ets_readonly::new(sequences.as_deref()).all() {
            let i = sequence.start();
            if i < next {
                continue;
            }
            let first = flow.transformations().at(i).clone();
            let second = flow.transformations().at(i + 1).clone();
            let (Some(a), Some(b)) =
                (classify_map_map_error_step(tp, &first), classify_map_map_error_step(tp, &second))
            else {
                // Rust: unreachable, both steps passed the candidate predicate (Go reads the zero value).
                continue;
            };
            if a.failure == b.failure || a.eager != b.eager {
                continue;
            }
            next = i + 2;

            let mut replacement = "mapBoth";
            if a.eager {
                replacement = "mapBothEager";
            }
            matches.push(MapMapErrorToMapBothMatch {
                source_file: sf,
                flow: flow.clone(),
                start: i,
                location: ets_shim::scanner::get_error_range_for_node(sf, second.callee()),
                first_name: a.name,
                second_name: b.name,
                replacement_name: replacement,
                first_failure: a.failure,
                // The fix removes one callee and replaces the other, so both must be
                // entity names whose evaluation runs no code.
                fixable: ast::is_entity_name_expression(first.callee())
                    && ast::is_entity_name_expression(second.callee())
                    && !has_explicit_type_arguments(Some(&second))
                    && !map_map_error_path_has_type_arguments(&first, &second)
                    && map_map_error_keeps_evaluation_order(&first, &second, a.eager),
                first,
                second,
            });
        }
    }
    matches
}

// go: internal/rules/map_map_error_to_map_both.go:127 mapMapErrorPathHasTypeArguments 03ab74be7e21150c
// mapMapErrorPathHasTypeArguments reports explicit type arguments on any call
// from the first step up to the call that applies the second step: the first
// step itself, its curried application or containing pipe, empty pipe calls in
// between, and the second step's application or containing pipe. Removing the
// first step changes the input type of every one of them, so their type
// arguments would no longer match. It also reports true when the second step's
// application is not an ancestor of the first step.
pub(crate) fn map_map_error_path_has_type_arguments(
    first: &PipingFlowTransformation,
    second: &PipingFlowTransformation,
) -> bool {
    let application = map_map_error_step_application(second);
    let mut node = Some(first.callee());
    while let Some(n) = node {
        if n.kind() == Kind::CallExpression
            && n.as_call_expression()
                .type_arguments()
                .is_some_and(|type_arguments| !type_arguments.nodes().is_empty())
        {
            return true;
        }
        if Some(n) == application {
            return false;
        }
        node = n.parent();
    }
    true
}

// go: internal/rules/map_map_error_to_map_both.go:144 mapMapErrorStepApplication 21960929190f40e5
// mapMapErrorStepApplication returns the call that applies a step to its input.
pub(crate) fn map_map_error_step_application(step: &PipingFlowTransformation) -> Option<P<Node>> {
    let call = step.callee().parent();
    let Some(call) = call else {
        return call;
    };
    if step.kind() != TransformationKind::Call && !is_piping_flow_argument_kind(step.kind()) {
        return Some(call);
    }
    // A curried step is applied by the call around its factory, and a pipe
    // argument by the pipe or Effect.fn call that holds it.
    let mut application = call.parent();
    while let Some(a) = application
        && a.kind() == Kind::ParenthesizedExpression
    {
        application = a.parent();
    }
    application
}

// go: internal/rules/map_map_error_to_map_both.go:163 mapMapErrorKeepsEvaluationOrder fd9411ca29a8e818
// mapMapErrorKeepsEvaluationOrder reports whether the fix keeps the observable
// order of evaluation. Steps in one argument list keep it: their callbacks are
// evaluated before any of them runs. Otherwise the fix moves the first callback
// into the second step and the first step's work into the merged step. Only a
// function literal can move across other code, because evaluating it runs none.
pub(crate) fn map_map_error_keeps_evaluation_order(
    first: &PipingFlowTransformation,
    second: &PipingFlowTransformation,
    eager: bool,
) -> bool {
    if map_map_error_share_argument_list(first, second) {
        return true;
    }
    if !is_function_literal(*first.args().at(0))
        && !map_map_error_first_callback_moves_freely(first, second)
    {
        return false;
    }
    // A non-eager step only builds an effect, so moving its work is unobservable.
    !eager || map_map_error_first_work_moves_freely(second)
}

// go: internal/rules/map_map_error_to_map_both.go:181 mapMapErrorFirstCallbackMovesFreely dca0e4af30e73a63
// mapMapErrorFirstCallbackMovesFreely reports whether nothing observable runs
// between the first callback's evaluation and the second step's callback
// position, apart from applying the first step. That holds when the first step
// evaluates its input before its callback and is the first step of its pipe,
// and the second step evaluates its input before its callback. A curried step
// evaluates its callback before its input, and earlier steps of the first pipe
// run before the second pipe's arguments are evaluated.
pub(crate) fn map_map_error_first_callback_moves_freely(
    first: &PipingFlowTransformation,
    second: &PipingFlowTransformation,
) -> bool {
    if second.kind() != TransformationKind::DataFirst
        && !is_piping_flow_argument_kind(second.kind())
    {
        return false;
    }
    if first.kind() == TransformationKind::DataFirst {
        return true;
    }
    if !is_piping_flow_argument_kind(first.kind()) {
        return false;
    }
    let (index, _) = map_map_error_argument_index(first);
    if first.kind() == TransformationKind::Pipeable {
        return index == 0;
    }
    // Function.pipe and Effect.fn hold their subject or body before the steps.
    index == 1
}

// go: internal/rules/map_map_error_to_map_both.go:204 mapMapErrorFirstWorkMovesFreely 81cd2659af69d389
// mapMapErrorFirstWorkMovesFreely reports whether an eager first step can run
// in the merged step without crossing observable code. When the first step is
// applied before the second callback is evaluated, the merged step runs that
// work after the second callback and, for a pipe argument, after the arguments
// that follow it.
pub(crate) fn map_map_error_first_work_moves_freely(second: &PipingFlowTransformation) -> bool {
    if second.kind() == TransformationKind::Call || second.kind() == TransformationKind::DataLast {
        true
    } else if second.kind() == TransformationKind::DataFirst {
        is_function_literal(*second.args().at(0))
    } else if is_piping_flow_argument_kind(second.kind()) {
        let (index, count) = map_map_error_argument_index(second);
        is_function_literal(*second.args().at(0)) && index == count - 1
    } else {
        false
    }
}

// go: internal/rules/map_map_error_to_map_both.go:219 mapMapErrorArgumentIndex 46038525148273f1
// mapMapErrorArgumentIndex returns the position of a pipe argument step in its
// call's argument list and the list's length, or -1 when it is not found.
//
// Rust: tsrs's call arguments are never nil, so that check drops.
pub(crate) fn map_map_error_argument_index(step: &PipingFlowTransformation) -> (i64, i64) {
    let Some(step_call) = step.callee().parent() else {
        return (-1, 0);
    };
    let Some(parent) = step_call.parent() else {
        return (-1, 0);
    };
    if parent.kind() != Kind::CallExpression {
        return (-1, 0);
    }
    let arguments = parent.as_call_expression().arguments().nodes();
    let index = arguments.iter().position(|&argument| argument == step_call);
    (index.map_or(-1, |index| index as i64), arguments.len() as i64)
}

// go: internal/rules/map_map_error_to_map_both.go:231 isFunctionLiteral 2c4467fc729d82cf
pub(crate) fn is_function_literal(node: P<Node>) -> bool {
    let node = ast::skip_parentheses(node);
    node.kind() == Kind::ArrowFunction || node.kind() == Kind::FunctionExpression
}

// go: internal/rules/map_map_error_to_map_both.go:236 mapMapErrorShareArgumentList 290e5f6b913af59e
pub(crate) fn map_map_error_share_argument_list(
    first: &PipingFlowTransformation,
    second: &PipingFlowTransformation,
) -> bool {
    if !is_piping_flow_argument_kind(first.kind()) || !is_piping_flow_argument_kind(second.kind()) {
        return false;
    }
    let (Some(first_step), Some(second_step)) = (first.callee().parent(), second.callee().parent())
    else {
        return false;
    };
    first_step.parent().is_some() && first_step.parent() == second_step.parent()
}

// go: internal/rules/map_map_error_to_map_both.go:244 classifyMapMapErrorStep 1742c20e554ea552
//
// Rust: Go's `(step, ok)` result is `Some(step)` when ok; a transformation's callee is never nil, so that check
// drops.
pub(crate) fn classify_map_map_error_step(
    tp: &mut TypeParser<'_>,
    step: &PipingFlowTransformation,
) -> Option<MapMapErrorStep> {
    if step.args().len() != 1 || step.args().at(0).kind() == Kind::SpreadElement {
        return None;
    }
    for candidate in &MAP_MAP_ERROR_STEPS {
        if tp.is_node_reference_to_effect_module_api(step.callee(), candidate.name) {
            return Some(*candidate);
        }
    }
    None
}
