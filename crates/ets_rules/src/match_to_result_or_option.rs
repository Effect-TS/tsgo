use std::cell::Cell;
use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile, SymbolFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    LazyExpressionFlags, ParsedLazyExpression, PipingFlow, PipingFlowSubject,
    PipingFlowTransformation, TransformationKind, TypeParser,
};

use crate::map_map_error_to_map_both::{
    is_function_literal, map_map_error_path_has_type_arguments,
};
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

// go: internal/rules/match_to_result_or_option.go:50 reifyingFamily 225eb613a8ce5dfd
// go: internal/rules/match_to_result_or_option.go:52 reifyingResult 25a2f52bafc0d9b3
// reifyingFamily is the container a handler reifies the outcome into.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ReifyingFamily {
    #[default]
    Result,
    Option,
    Either,
}

// go: internal/rules/match_to_result_or_option.go:58 reifyingReplacementNames 216d4e533de1bb2d
static REIFYING_REPLACEMENT_NAMES: [&str; 3] = [
    "result", // reifyingResult
    "option", // reifyingOption
    "either", // reifyingEither
];

// go: internal/rules/match_to_result_or_option.go:65 reifyingKind 17cca68dae08fd35
// reifyingKind is a constructor's container and whether it wraps the failure.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ReifyingKind {
    family: ReifyingFamily,
    failure: bool,
}

/// Rust-only: Go's `reifyingKind{family: reifyingOption, failure: true}` literal (Option.none).
const REIFYING_OPTION_NONE: ReifyingKind =
    ReifyingKind { family: ReifyingFamily::Option, failure: true };

// go: internal/rules/match_to_result_or_option.go:73 AnalyzeMatchToResultOrOption 1db7809acef9235e
// AnalyzeMatchToResultOrOption finds Effect.match and Effect.matchEffect steps
// whose handlers only wrap the value and the error, and adjacent Effect.map and
// Effect.catch (Effect.catchAll in v3) steps that do the same.
//
// Rust: Go's checker argument is reached through the parser and dropped.
pub fn analyze_match_to_result_or_option(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<MatchToResultOrOptionMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    let is_map = |tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| -> bool {
        has_single_argument(step) && tp.is_node_reference_to_effect_module_api(step.callee(), "map")
    };
    let is_catch = |tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| -> bool {
        match_to_result_catch_name(tp, step).is_some()
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (i, step) in flow.transformations().all() {
            let Some((kind, source_name)) = analyze_reifying_fold(tp, step) else {
                continue;
            };
            matches.push(new_match_to_result_or_option_match(
                tp,
                sf,
                flow,
                i,
                i,
                kind,
                source_name.to_string(),
            ));
        }
        let sequences =
            PipingFlow::find_transformation_sequences(Some(flow), tp, &[&is_map, &is_catch]);
        for (_, sequence) in ets_readonly::new(sequences.as_deref()).all() {
            let i = sequence.start();
            let (map_step, catch_step) =
                (flow.transformations().at(i), flow.transformations().at(i + 1));
            let Some(success) = reifying_handler(tp, *map_step.args().at(0)) else {
                continue;
            };
            if success.failure {
                continue;
            }
            let Some(failure) = succeed_reifying_handler(tp, *catch_step.args().at(0)) else {
                continue;
            };
            if !failure.failure || failure.family != success.family {
                continue;
            }
            let catch_name = match_to_result_catch_name(tp, catch_step).unwrap_or_default();
            matches.push(new_match_to_result_or_option_match(
                tp,
                sf,
                flow,
                i,
                i + 1,
                success,
                "map/".to_string() + catch_name,
            ));
        }
    }
    matches
}

// go: internal/rules/match_to_result_or_option.go:112 newMatchToResultOrOptionMatch 43a905160f357542
pub(crate) fn new_match_to_result_or_option_match(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    flow: &PipingFlow,
    start: i64,
    end: i64,
    kind: ReifyingKind,
    source_name: String,
) -> MatchToResultOrOptionMatch {
    MatchToResultOrOptionMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(
            sf,
            flow.transformations().at(end).callee(),
        ),
        flow: flow.clone(),
        start,
        end,
        replacement_name: REIFYING_REPLACEMENT_NAMES[kind.family as usize],
        source_name,
        fixable: match_to_result_or_option_fixable(tp, flow, start, end),
    }
}

// go: internal/rules/match_to_result_or_option.go:128 analyzeReifyingFold c9864e80c9902745
// analyzeReifyingFold classifies an Effect.match or Effect.matchEffect step
// whose options are exactly an onFailure and an onSuccess handler that wrap
// the error and the value into the same container.
//
// Rust: Go's `(kind, sourceName, ok)` is `Some((kind, sourceName))` when ok.
pub(crate) fn analyze_reifying_fold(
    tp: &mut TypeParser<'_>,
    step: &PipingFlowTransformation,
) -> Option<(ReifyingKind, &'static str)> {
    if !has_single_argument(step) {
        return None;
    }
    let (Some(on_failure), Some(on_success)) = match_handler_pair(*step.args().at(0)) else {
        return None;
    };
    let classify: fn(&mut TypeParser<'_>, P<Node>) -> Option<ReifyingKind>;
    let source_name;
    if tp.is_node_reference_to_effect_module_api(step.callee(), "match") {
        classify = reifying_handler;
        source_name = "match";
    } else if tp.is_node_reference_to_effect_module_api(step.callee(), "matchEffect") {
        classify = succeed_reifying_handler;
        source_name = "matchEffect";
    } else {
        return None;
    }
    let failure = classify(tp, on_failure)?;
    if !failure.failure {
        return None;
    }
    let success = classify(tp, on_success)?;
    if success.failure || success.family != failure.family {
        return None;
    }
    Some((success, source_name))
}

// go: internal/rules/match_to_result_or_option.go:161 matchHandlerPair 1e43b7f93a945a74
// matchHandlerPair returns the handlers of an object literal whose only
// properties are onFailure and onSuccess assignments.
//
// Rust: tsrs's object literal properties are never nil, so that check drops.
pub(crate) fn match_handler_pair(node: P<Node>) -> (Option<P<Node>>, Option<P<Node>>) {
    let node = ast::skip_parentheses(node);
    if node.kind() != Kind::ObjectLiteralExpression {
        return (None, None);
    }
    let properties = node.as_object_literal_expression().properties();
    if properties.nodes().len() != 2 {
        return (None, None);
    }
    let (mut on_failure, mut on_success) = (None, None);
    for &property in properties.nodes() {
        if property.kind() != Kind::PropertyAssignment {
            return (None, None);
        }
        let Some(name) = property.name() else {
            return (None, None);
        };
        if name.kind() != Kind::Identifier && name.kind() != Kind::StringLiteral {
            return (None, None);
        }
        match name.text() {
            "onFailure" => on_failure = property.initializer(),
            "onSuccess" => on_success = property.initializer(),
            _ => {}
        }
    }
    (on_failure, on_success)
}

// go: internal/rules/match_to_result_or_option.go:191 reifyingHandler f033bfe500ec0f36
// reifyingHandler classifies a handler that wraps its argument into a
// container: the constructor itself, a forwarder to it, or a function whose
// body is exactly Option.none().
pub(crate) fn reifying_handler(tp: &mut TypeParser<'_>, node: P<Node>) -> Option<ReifyingKind> {
    let (target, type_arguments, _) = tp.unwrap_identity_forwarder(node);
    if type_arguments.is_some_and(|type_arguments| !type_arguments.nodes().is_empty()) {
        return None;
    }
    if let Some(kind) = reifying_constructor(tp, target) {
        return Some(kind);
    }
    let lazy = ets_typeparser::parse_lazy_expression(node, LazyExpressionFlags::None);
    if let Some(lazy) = lazy
        && has_plain_parameters(&lazy)
        && is_reifying_none_call(tp, Some(lazy.expression()))
    {
        return Some(REIFYING_OPTION_NONE);
    }
    None
}

// go: internal/rules/match_to_result_or_option.go:212 succeedReifyingHandler d4ee085f04f69fda
// succeedReifyingHandler classifies a handler whose body lifts a reified
// argument with Effect.succeed: Effect.succeed(Result.fail(e)) for its
// parameter e, or Effect.succeed(Option.none()). Only direct calls count: a
// pipe or pipeable step may run code through its pipe callee or receiver
// (getFunction().pipe(...), e.pipe(...)), and a step does not expose its
// enclosing pipe call to check that callee.
//
// Rust: the step predicate records the constructor's kind in a `Cell` (Go assigns the captured variable).
pub(crate) fn succeed_reifying_handler(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> Option<ReifyingKind> {
    let lazy = ets_typeparser::parse_lazy_expression(node, LazyExpressionFlags::None)?;
    if !has_plain_parameters(&lazy) {
        return None;
    }
    let flow = tp.longest_piping_flow_at(lazy.expression(), false);
    let is_succeed = |tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| -> bool {
        let callee = ast::skip_parentheses(step.callee());
        is_direct_reifying_call(step)
            && is_side_effect_free_reference(tp, Some(callee))
            && tp.is_node_reference_to_effect_module_api(callee, "succeed")
    };
    if PipingFlow::matches_exactly(
        flow.as_ref(),
        tp,
        &|tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
            is_reifying_none_call(tp, subject.node())
        },
        &[&is_succeed],
    ) {
        return Some(REIFYING_OPTION_NONE);
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
                if !is_direct_reifying_call(step) {
                    return false;
                }
                let constructor = reifying_constructor(tp, Some(step.callee()));
                kind.set(constructor.unwrap_or_default());
                constructor.is_some_and(|constructor| constructor != REIFYING_OPTION_NONE)
            },
            &is_succeed,
        ],
    );
    matched.then_some(kind.get())
}

// go: internal/rules/match_to_result_or_option.go:244 isDirectReifyingCall 6723f2ca2f97d791
// isDirectReifyingCall reports whether a handler body step is a plain call
// such as Effect.succeed(x), without explicit type arguments.
pub(crate) fn is_direct_reifying_call(step: &PipingFlowTransformation) -> bool {
    step.kind() == TransformationKind::Call
        && step.args().len() == 0
        && !has_explicit_type_arguments(Some(step))
}

// go: internal/rules/match_to_result_or_option.go:253 isSideEffectFreeReference a064ce13f0404e35
// isSideEffectFreeReference reports whether evaluating node runs no code, so
// dropping the handler or step that holds it changes nothing: an identifier,
// or a property read through module namespaces only. Other receivers may run
// code while resolving to the same export, such as the call in
// getOption().none or a getter in modules.option.none.
pub(crate) fn is_side_effect_free_reference(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> bool {
    let Some(node) = node else {
        return false;
    };
    match node.kind() {
        Kind::Identifier => true,
        Kind::PropertyAccessExpression => {
            let access = node.as_property_access_expression();
            access.question_dot_token().is_none()
                && is_module_namespace_reference(tp, ast::skip_parentheses(access.expression()))
        }
        _ => false,
    }
}

// go: internal/rules/match_to_result_or_option.go:269 isModuleNamespaceReference 42ac062380695846
// isModuleNamespaceReference reports whether node is a side-effect-free
// reference to a module namespace, such as an import of effect/Option.
pub(crate) fn is_module_namespace_reference(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    if !is_side_effect_free_reference(tp, Some(node)) {
        return false;
    }
    let mut symbol = tp.get_symbol_at_location(node);
    if let Some(s) = symbol
        && s.flags().intersects(SymbolFlags::Alias)
    {
        symbol = Some(tp.checker().get_aliased_symbol(s));
    }
    symbol.is_some_and(|symbol| symbol.flags().intersects(SymbolFlags::ValueModule))
}

// go: internal/rules/match_to_result_or_option.go:282 reifyingConstructor b0b56e72bb0738bc
// reifyingConstructor classifies a side-effect-free reference to a
// constructor that wraps a value or an error into a Result, Option or Either.
//
// Rust: Go's `(kind, ok)` is `Some(kind)` when ok.
pub(crate) fn reifying_constructor(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> Option<ReifyingKind> {
    let node = node.map(ast::skip_parentheses);
    if !is_side_effect_free_reference(tp, node) {
        return None;
    }
    let node = node?;
    if tp.is_node_reference_to_effect_result_module_api(node, "succeed") {
        Some(ReifyingKind { family: ReifyingFamily::Result, failure: false })
    } else if tp.is_node_reference_to_effect_result_module_api(node, "fail") {
        Some(ReifyingKind { family: ReifyingFamily::Result, failure: true })
    } else if tp.is_node_reference_to_effect_option_module_api(node, "some") {
        Some(ReifyingKind { family: ReifyingFamily::Option, failure: false })
    } else if tp.is_node_reference_to_effect_option_module_api(node, "none") {
        Some(ReifyingKind { family: ReifyingFamily::Option, failure: true })
    } else if tp.is_node_reference_to_effect_either_module_api(node, "right") {
        Some(ReifyingKind { family: ReifyingFamily::Either, failure: false })
    } else if tp.is_node_reference_to_effect_either_module_api(node, "left") {
        Some(ReifyingKind { family: ReifyingFamily::Either, failure: true })
    } else {
        None
    }
}

// go: internal/rules/match_to_result_or_option.go:306 isReifyingNoneCall f5a83ca372d4bb4b
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
    reifying_constructor(tp, Some(call.expression())) == Some(REIFYING_OPTION_NONE)
}

// go: internal/rules/match_to_result_or_option.go:321 hasPlainParameters 2d488e95f01f3bed
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

// go: internal/rules/match_to_result_or_option.go:335 matchToResultCatchName 1f20dae9b1e52ad2
//
// Rust: Go's `(name, ok)` is `Some(name)` when ok.
pub(crate) fn match_to_result_catch_name(
    tp: &mut TypeParser<'_>,
    step: &PipingFlowTransformation,
) -> Option<&'static str> {
    if !has_single_argument(step) || !is_function_literal(*step.args().at(0)) {
        return None;
    }
    ["catch", "catchAll"]
        .into_iter()
        .find(|&name| tp.is_node_reference_to_effect_module_api(step.callee(), name))
}

// go: internal/rules/match_to_result_or_option.go:347 hasSingleArgument 451e241223308f8f
pub(crate) fn has_single_argument(step: &PipingFlowTransformation) -> bool {
    step.args().len() == 1
}

// go: internal/rules/match_to_result_or_option.go:356 matchToResultOrOptionFixable 550d1c0cc2902336
// matchToResultOrOptionFixable reports whether the replaced steps can be
// dropped: their callees run no code when evaluated, and no explicit type
// argument depends on the old output type. The dropped handlers are always
// side-effect-free references or function literals, as the classifiers
// require.
pub(crate) fn match_to_result_or_option_fixable(
    tp: &mut TypeParser<'_>,
    flow: &PipingFlow,
    start: i64,
    end: i64,
) -> bool {
    for i in start..=end {
        let step = flow.transformations().at(i);
        if !is_side_effect_free_reference(tp, Some(step.callee())) {
            return false;
        }
    }
    !map_map_error_path_has_type_arguments(
        flow.transformations().at(start),
        flow.transformations().at(end),
    )
}
