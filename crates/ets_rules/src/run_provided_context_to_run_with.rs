use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectMajorVersion, PipingFlow, PipingFlowTransformation, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_0_OF_EFFECT_1_RE_IMPLEMENTS_EFFECT_2_USE_EFFECT_2_CONTEXT_EFFECT_TO_RUN_THE_EFFECT_WITH_THE_CONTEXT_DIRECTLY_EFFECT_RUN_PROVIDED_CONTEXT_TO_RUN_WITH;

// go: internal/rules/run_provided_context_to_run_with.go:18 RunProvidedContextToRunWith 3f3eac92f18a6785
// RunProvidedContextToRunWith suggests the dedicated Effect.run*With runners
// instead of running an effect whose outermost step provides a Context.
pub static RUN_PROVIDED_CONTEXT_TO_RUN_WITH: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "runProvidedContextToRunWith",
    group: "style",
    description: "Suggests Effect.run*With instead of running an effect provided with Effect.provideContext or Effect.setContext",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_run_provided_context_to_run_with(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.runner_name, m.provide_name, m.replacement_name],
            ));
        }
        diagnostics
    },
});

// go: internal/rules/run_provided_context_to_run_with.go:47 RunProvidedContextToRunWithMatch be9320961afb840e
// RunProvidedContextToRunWithMatch holds the transformations needed by the
// diagnostic and quick fix.
//
// Rust: Go's `*PipingFlow` and `*PipingFlowTransformation` (pointers to the cached flow and to local copies) are
// owned copies; Go never stores nil in them, nor in RunnerCallee or Context, so those fields are not optional.
#[derive(Clone)]
pub struct RunProvidedContextToRunWithMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub flow: PipingFlow,
    pub provide_index: i64,
    pub provide: PipingFlowTransformation,
    pub runner: PipingFlowTransformation,
    pub runner_callee: P<Node>,
    // Context is the Context.Context argument of the provide step.
    pub context: P<Node>,
    // EffectModuleNode is the receiver of RunnerCallee when it is the Effect
    // module, so the fix keeps namespace aliases; nil otherwise.
    pub effect_module_node: Option<P<Node>>,
    pub runner_name: &'static str,
    pub provide_name: &'static str,
    pub replacement_name: &'static str,
}

// go: internal/rules/run_provided_context_to_run_with.go:65 runProvidedContextRunners 07b31613ff157fc5
//
// Rust: Go's anonymous `{runner, replacement}` struct is a `(runner, replacement)` tuple.
pub(crate) static RUN_PROVIDED_CONTEXT_RUNNERS: &[(&str, &str)] = &[
    ("runPromise", "runPromiseWith"),
    ("runPromiseExit", "runPromiseExitWith"),
    ("runSync", "runSyncWith"),
    ("runSyncExit", "runSyncExitWith"),
    ("runFork", "runForkWith"),
    ("runCallback", "runCallbackWith"),
];

// go: internal/rules/run_provided_context_to_run_with.go:81 AnalyzeRunProvidedContextToRunWith 846b378388277d69
// AnalyzeRunProvidedContextToRunWith finds Effect.provideContext or
// Effect.setContext immediately followed by a non-*With runner in a normalized
// piping flow, including Effect.fn steps. Adjacency is important: a provide
// step followed by another transformation is not the runner's input.
//
// Rust: Go's checker argument is the parser's checker; Go ignores it, so it is dropped.
pub fn analyze_run_provided_context_to_run_with(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<RunProvidedContextToRunWithMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        let sequences = PipingFlow::find_transformation_sequences(
            Some(flow),
            tp,
            &[
                &|tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
                    !run_provided_context_provide_name(tp, transformation).is_empty()
                },
                &|tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
                    let (runner_name, _) = run_provided_context_runner(tp, transformation);
                    !runner_name.is_empty()
                },
            ],
        );
        for sequence in sequences.iter().flatten() {
            let provide = flow.transformations().at(sequence.start()).clone();
            let runner = flow.transformations().at(sequence.start() + 1).clone();
            let context = *provide.args().at(0);
            if tp.context_type_at_location(context).is_none() {
                continue;
            }
            let (runner_name, replacement_name) = run_provided_context_runner(tp, &runner);
            let runner_callee = runner.callee();

            let mut effect_module_node = None;
            if runner_callee.kind() == Kind::PropertyAccessExpression {
                let receiver = runner_callee.as_property_access_expression().expression();
                if tp.is_expression_effect_module(receiver) {
                    effect_module_node = Some(receiver);
                }
            }

            let provide_name = run_provided_context_provide_name(tp, &provide);
            matches.push(RunProvidedContextToRunWithMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, runner_callee),
                flow: flow.clone(),
                provide_index: sequence.start(),
                provide,
                runner,
                runner_callee,
                context,
                effect_module_node,
                runner_name,
                provide_name,
                replacement_name,
            });
        }
    }

    ets_go::sort::slice(&mut matches, |a, b| a.location.pos() < b.location.pos());
    matches
}

// go: internal/rules/run_provided_context_to_run_with.go:139 runProvidedContextProvideName 16c9cc921bf0ebd7
// runProvidedContextProvideName returns "provideContext" or "setContext" when
// transformation provides a single argument through one of those APIs.
pub(crate) fn run_provided_context_provide_name(
    tp: &mut TypeParser<'_>,
    transformation: &PipingFlowTransformation,
) -> &'static str {
    if transformation.args().len() != 1 {
        return "";
    }
    for name in ["provideContext", "setContext"] {
        if tp.is_node_reference_to_effect_module_api(transformation.callee(), name) {
            return name;
        }
    }
    ""
}

// go: internal/rules/run_provided_context_to_run_with.go:153 runProvidedContextRunner a7445846ec94df7e
// runProvidedContextRunner returns the runner name and its *With replacement
// when transformation runs its input with one of the context-free runners.
pub(crate) fn run_provided_context_runner(
    tp: &mut TypeParser<'_>,
    transformation: &PipingFlowTransformation,
) -> (&'static str, &'static str) {
    for &(runner, replacement) in RUN_PROVIDED_CONTEXT_RUNNERS {
        if tp.is_node_reference_to_effect_module_api(transformation.callee(), runner) {
            return (runner, replacement);
        }
    }
    ("", "")
}
