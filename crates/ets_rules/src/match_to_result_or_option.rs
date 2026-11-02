use std::cell::Cell;
use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    LazyExpressionFlags, ParsedLazyExpression, PipingFlow, PipingFlowSubject,
    PipingFlowTransformation, TypeParser,
};

use crate::map_map_error_to_map_both::map_map_error_path_has_type_arguments;
use crate::option_match_to_from_option::same_identifier_reference;
use crate::piping_flow::has_explicit_type_arguments;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_0_REIFIES_THE_OUTCOME_DIRECTLY_THESE_EFFECT_1_HANDLERS_ONLY_WRAP_THE_VALUE_AND_THE_ERROR_EFFECT_MATCH_TO_RESULT_OR_OPTION;

// go: internal/rules/match_to_result_or_option.go:16 MatchToResultOrOption 6478a48b9400d941
// MatchToResultOrOption suggests Effect.result, Effect.option or Effect.either
// when match handlers, or adjacent map and catch steps, only reify the outcome.
pub static MATCH_TO_RESULT_OR_OPTION: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "matchToResultOrOption",
    group: "style",
    description: "Suggests Effect.result, Effect.option or Effect.either when match handlers, or a map step followed by a catch step, only reify the outcome",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_match_to_result_or_option(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.replacement_name, &m.source_name],
            ));
        }
        diagnostics
    },
});

// go: internal/rules/match_to_result_or_option.go:37 MatchToResultOrOptionMatch 5569f9e7e6bc3842
#[derive(Clone)]
pub struct MatchToResultOrOptionMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub flow: PipingFlow,
    // Start and End are the inclusive range of steps the replacement covers:
    // the fold step alone, or the map and catch pair.
    pub start: i64,
    pub end: i64,
    pub replacement_name: &'static str, // result, option or either
    pub source_name: String,            // match, matchEffect, map/catch or map/catchAll
    pub fixable: bool,
}

// go: internal/rules/match_to_result_or_option.go:51 reifyingKind 790565b4fbea0540
// reifyingKind is the container a constructor wraps into (result, option or
// either) and whether it wraps the failure.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ReifyingKind {
    replacement: &'static str,
    failure: bool,
}

// go: internal/rules/match_to_result_or_option.go:56 reifyingNone 89ffc2d02b21376f
const REIFYING_NONE: ReifyingKind = ReifyingKind { replacement: "option", failure: true };

/// Rust-only: Go's `classify` handler classifiers share this signature.
type ReifyingClassifier = fn(&mut TypeParser<'_>, Option<P<Node>>) -> Option<ReifyingKind>;

// go: internal/rules/match_to_result_or_option.go:61 AnalyzeMatchToResultOrOption a4b50a8a6a17e690
// AnalyzeMatchToResultOrOption finds Effect.match and Effect.matchEffect steps
// whose handlers only wrap the value and the error, and adjacent Effect.map and
// Effect.catch (Effect.catchAll in v3) steps that do the same.
//
// Rust: Go's checker argument is reached through the parser and dropped; Go's
// `add` closure is `push_match_to_result_or_option_match`.
pub fn analyze_match_to_result_or_option(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<MatchToResultOrOptionMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    let is_fold = |tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| -> bool {
        step.args().len() == 1
            && (tp.is_node_reference_to_effect_module_api(step.callee(), "match")
                || tp.is_node_reference_to_effect_module_api(step.callee(), "matchEffect"))
    };
    let is_map = |tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| -> bool {
        step.args().len() == 1 && tp.is_node_reference_to_effect_module_api(step.callee(), "map")
    };
    let is_catch = |tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| -> bool {
        step.args().len() == 1 && !match_to_result_catch_name(tp, step).is_empty()
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        let sequences = PipingFlow::find_transformation_sequences(Some(flow), tp, &[&is_fold]);
        for (_, sequence) in ets_readonly::new(sequences.as_deref()).all() {
            let i = sequence.start();
            let step = flow.transformations().at(i);
            let options = ast::skip_parentheses(*step.args().at(0));
            // Exactly two properties: a spread or any other member could
            // replace or add to the onFailure and onSuccess handlers.
            //
            // Rust: tsrs's object literal properties are never nil, so that check drops.
            if options.kind() != Kind::ObjectLiteralExpression
                || options.as_object_literal_expression().properties().nodes().len() != 2
            {
                continue;
            }
            let (mut classify, mut source_name): (ReifyingClassifier, &str) =
                (reifying_handler, "match");
            if tp.is_node_reference_to_effect_module_api(step.callee(), "matchEffect") {
                (classify, source_name) = (succeed_reifying_handler, "matchEffect");
            }
            let Some(on_failure) = classify(
                tp,
                ets_typeparser::object_literal_property_initializer(options, "onFailure"),
            ) else {
                continue;
            };
            if let Some(on_success) = classify(
                tp,
                ets_typeparser::object_literal_property_initializer(options, "onSuccess"),
            ) {
                push_match_to_result_or_option_match(
                    &mut matches,
                    sf,
                    flow,
                    i,
                    i,
                    on_failure,
                    on_success,
                    source_name.to_string(),
                );
            }
        }
        let sequences =
            PipingFlow::find_transformation_sequences(Some(flow), tp, &[&is_map, &is_catch]);
        for (_, sequence) in ets_readonly::new(sequences.as_deref()).all() {
            let i = sequence.start();
            let (map_step, catch_step) =
                (flow.transformations().at(i), flow.transformations().at(i + 1));
            let Some(on_success) = reifying_handler(tp, Some(*map_step.args().at(0))) else {
                continue;
            };
            if let Some(on_failure) = succeed_reifying_handler(tp, Some(*catch_step.args().at(0))) {
                let source_name = "map/".to_string() + match_to_result_catch_name(tp, catch_step);
                push_match_to_result_or_option_match(
                    &mut matches,
                    sf,
                    flow,
                    i,
                    i + 1,
                    on_failure,
                    on_success,
                    source_name,
                );
            }
        }
    }
    matches
}

/// Rust-only: Go's `add` closure in `AnalyzeMatchToResultOrOption`.
#[allow(clippy::too_many_arguments)]
fn push_match_to_result_or_option_match(
    matches: &mut Vec<MatchToResultOrOptionMatch>,
    sf: P<SourceFile>,
    flow: &PipingFlow,
    start: i64,
    end: i64,
    on_failure: ReifyingKind,
    on_success: ReifyingKind,
    source_name: String,
) {
    if !on_failure.failure || on_success.failure || on_failure.replacement != on_success.replacement
    {
        return;
    }
    matches.push(MatchToResultOrOptionMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(
            sf,
            flow.transformations().at(end).callee(),
        ),
        flow: flow.clone(),
        start,
        end,
        replacement_name: on_success.replacement,
        source_name,
        fixable: !map_map_error_path_has_type_arguments(
            flow.transformations().at(start),
            flow.transformations().at(end),
        ),
    });
}

// go: internal/rules/match_to_result_or_option.go:133 reifyingHandler 4e1408a343c8af76
// reifyingHandler classifies a handler that wraps its argument into a
// container: the constructor itself, an identity forwarder to it, or a
// function whose body is Option.none().
//
// Rust: Go's `(kind, ok)` is `Some(kind)` when ok; the unused checker drops.
pub(crate) fn reifying_handler(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> Option<ReifyingKind> {
    let node = node?;
    let (target, type_arguments, _) = tp.unwrap_identity_forwarder(node);
    if let Some(kind) = reifying_constructor(tp, target) {
        return type_arguments
            .is_none_or(|type_arguments| type_arguments.nodes().is_empty())
            .then_some(kind);
    }
    let lazy = ets_typeparser::parse_lazy_expression(node, LazyExpressionFlags::None);
    let matched = lazy.is_some_and(|lazy| {
        has_plain_parameters(&lazy) && is_reifying_none_call(tp, Some(lazy.expression()))
    });
    matched.then_some(REIFYING_NONE)
}

// go: internal/rules/match_to_result_or_option.go:148 succeedReifyingHandler 2df450b484148fdc
// succeedReifyingHandler classifies a handler whose body lifts a reified
// value with Effect.succeed: Effect.succeed(Result.fail(e)) for its parameter
// e, or Effect.succeed(Option.none()), in any flow spelling.
//
// Rust: the step predicate records the constructor's kind in a `Cell` (Go assigns the captured variable).
pub(crate) fn succeed_reifying_handler(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> Option<ReifyingKind> {
    let lazy = ets_typeparser::parse_lazy_expression(node?, LazyExpressionFlags::None)?;
    if !has_plain_parameters(&lazy) {
        return None;
    }
    let flow = tp.longest_piping_flow_at(lazy.expression(), false);
    let is_plain_step = |step: &PipingFlowTransformation| -> bool {
        step.args().len() == 0 && !has_explicit_type_arguments(Some(step))
    };
    let is_succeed = |tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| -> bool {
        is_plain_step(step) && tp.is_node_reference_to_effect_module_api(step.callee(), "succeed")
    };
    if PipingFlow::matches_exactly(
        flow.as_ref(),
        tp,
        &|tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
            is_reifying_none_call(tp, subject.node())
        },
        &[&is_succeed],
    ) {
        return Some(REIFYING_NONE);
    }
    let kind = Cell::new(ReifyingKind::default());
    let matched = PipingFlow::matches_exactly(
        flow.as_ref(),
        tp,
        &|tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
            lazy.params().len() == 1
                && same_identifier_reference(
                    tp,
                    lazy.params().at(0).name(),
                    subject.node().map(ast::skip_parentheses),
                )
        },
        &[
            &|tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| {
                let constructor = reifying_constructor(tp, Some(step.callee()));
                kind.set(constructor.unwrap_or_default());
                constructor
                    .is_some_and(|constructor| is_plain_step(step) && constructor != REIFYING_NONE)
            },
            &is_succeed,
        ],
    );
    matched.then_some(kind.get())
}

// go: internal/rules/match_to_result_or_option.go:181 reifyingConstructor 72a5b36652f6f49c
// reifyingConstructor classifies a reference to a constructor that wraps a
// value or an error into a Result, Option or Either.
//
// Rust: Go's `(kind, ok)` is `Some(kind)` when ok.
pub(crate) fn reifying_constructor(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> Option<ReifyingKind> {
    let node = node?;
    if tp.is_node_reference_to_effect_result_module_api(node, "succeed") {
        Some(ReifyingKind { replacement: "result", failure: false })
    } else if tp.is_node_reference_to_effect_result_module_api(node, "fail") {
        Some(ReifyingKind { replacement: "result", failure: true })
    } else if tp.is_node_reference_to_effect_option_module_api(node, "some") {
        Some(ReifyingKind { replacement: "option", failure: false })
    } else if tp.is_node_reference_to_effect_option_module_api(node, "none") {
        Some(REIFYING_NONE)
    } else if tp.is_node_reference_to_effect_either_module_api(node, "right") {
        Some(ReifyingKind { replacement: "either", failure: false })
    } else if tp.is_node_reference_to_effect_either_module_api(node, "left") {
        Some(ReifyingKind { replacement: "either", failure: true })
    } else {
        None
    }
}

// go: internal/rules/match_to_result_or_option.go:201 isReifyingNoneCall 1ae5307dadd929d9
// isReifyingNoneCall reports whether node is Option.none(), with or without
// explicit type arguments.
//
// Rust: tsrs's call arguments are never nil, so that check drops.
pub(crate) fn is_reifying_none_call(tp: &mut TypeParser<'_>, node: Option<P<Node>>) -> bool {
    let Some(node) = node.map(ast::skip_parentheses) else {
        return false;
    };
    if node.kind() != Kind::CallExpression {
        return false;
    }
    let call = node.as_call_expression();
    if !call.arguments().nodes().is_empty() {
        return false;
    }
    reifying_constructor(tp, Some(call.expression())) == Some(REIFYING_NONE)
}

// go: internal/rules/match_to_result_or_option.go:216 hasPlainParameters 2d488e95f01f3bed
// hasPlainParameters reports whether a handler declares at most one
// parameter, bound to an identifier without a default or rest marker.
pub(crate) fn has_plain_parameters(lazy: &ParsedLazyExpression) -> bool {
    if lazy.params().len() > 1 {
        return false;
    }
    for (_, &parameter) in lazy.params().all() {
        let declaration = parameter.as_parameter_declaration();
        if declaration.name().kind() != Kind::Identifier
            || declaration.dot_dot_dot_token().is_some()
            || declaration.initializer().is_some()
        {
            return false;
        }
    }
    true
}

// go: internal/rules/match_to_result_or_option.go:232 matchToResultCatchName c474ed6b4ca700f3
// matchToResultCatchName returns catch or catchAll for an Effect catch step,
// and "" for any other step.
pub(crate) fn match_to_result_catch_name(
    tp: &mut TypeParser<'_>,
    step: &PipingFlowTransformation,
) -> &'static str {
    ["catch", "catchAll"]
        .into_iter()
        .find(|&name| tp.is_node_reference_to_effect_module_api(step.callee(), name))
        .unwrap_or_default()
}
