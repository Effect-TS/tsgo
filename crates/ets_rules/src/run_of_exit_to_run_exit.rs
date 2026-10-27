use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{PipingFlow, PipingFlowTransformation, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::EFFECT_0_OF_EFFECT_EXIT_RE_IMPLEMENTS_EFFECT_1_USE_THE_DEDICATED_EXIT_RUNNER_DIRECTLY_EFFECT_RUN_OF_EXIT_TO_RUN_EXIT;

// go: internal/rules/run_of_exit_to_run_exit.go:18 RunOfExitToRunExit
// RunOfExitToRunExit suggests using the dedicated promise Exit runner instead
// of running an Effect that has first been transformed with Effect.exit.
pub static RUN_OF_EXIT_TO_RUN_EXIT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "runOfExitToRunExit",
    group: "style",
    description: "Suggests using Effect.runPromiseExit instead of passing Effect.exit to Effect.runPromise",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_run_of_exit_to_run_exit(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.runner_name, m.replacement_name],
            ));
        }
        diagnostics
    },
});

// go: internal/rules/run_of_exit_to_run_exit.go:46 RunOfExitToRunExitMatch
// RunOfExitToRunExitMatch holds the transformations needed by the diagnostic
// and quick fix.
//
// Rust: Go's `*PipingFlowTransformation` (a pointer to a local copy) is an owned copy.
#[derive(Clone)]
pub struct RunOfExitToRunExitMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub exit_transformation: PipingFlowTransformation,
    pub runner_callee: Option<P<Node>>,
    pub runner_name: &'static str,
    pub replacement_name: &'static str,
}

// go: internal/rules/run_of_exit_to_run_exit.go:58 AnalyzeRunOfExitToRunExit
// AnalyzeRunOfExitToRunExit finds Effect.exit immediately followed by a
// non-Exit runner in a normalized piping flow. Adjacency is important: an
// Effect.exit followed by another transformation is not the runner's input.
//
// Rust: Go's checker argument is the parser's checker; Go ignores it, so it is dropped.
pub fn analyze_run_of_exit_to_run_exit(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<RunOfExitToRunExitMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, false);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        let sequences = PipingFlow::find_transformation_sequences(
            Some(flow),
            tp,
            &[
                &|tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
                    transformation.args().len() == 0
                        && tp
                            .is_node_reference_to_effect_module_api(transformation.callee(), "exit")
                },
                &|tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
                    let (_, _, runner_name, _) =
                        run_of_exit_runner(tp, Some(transformation.callee()));
                    !runner_name.is_empty()
                },
            ],
        );
        for sequence in sequences.iter().flatten() {
            let exit_transformation = flow.transformations().at(sequence.start()).clone();
            let runner_transformation = flow.transformations().at(sequence.start() + 1);
            let (runner_callee, runner_name_node, runner_name, replacement_name) =
                run_of_exit_runner(tp, Some(runner_transformation.callee()));

            append_run_of_exit_match(
                &mut matches,
                RunOfExitToRunExitMatch {
                    source_file: sf,
                    location: run_of_exit_location(
                        sf,
                        runner_transformation.callee(),
                        runner_name_node,
                    ),
                    exit_transformation,
                    runner_callee,
                    runner_name,
                    replacement_name,
                },
            );
        }
    }

    // Non-dual runners with RunOptions are not piping-flow transformations,
    // because their effect parameter is one of multiple arguments. Inspect the
    // runner call itself, then still use a piping flow for its effect argument so
    // direct Effect.exit calls and pipe tails share the same matching logic.
    //
    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<RunOfExitToRunExitMatch>,
    ) {
        if node.kind() == Kind::CallExpression {
            let call = node.as_call_expression();
            let arguments = call.arguments().nodes();
            if !arguments.is_empty() {
                let (runner_callee, runner_name_node, runner_name, replacement_name) =
                    run_of_exit_runner(tp, Some(call.expression()));
                if !runner_name.is_empty()
                    && let Some(argument_flow) = tp.longest_piping_flow_at(arguments[0], false)
                    && argument_flow.transformations().len() > 0
                {
                    let exit_transformation = argument_flow
                        .transformations()
                        .at(argument_flow.transformations().len() - 1)
                        .clone();
                    if exit_transformation.args().len() == 0
                        && tp.is_node_reference_to_effect_module_api(
                            exit_transformation.callee(),
                            "exit",
                        )
                    {
                        append_run_of_exit_match(
                            matches,
                            RunOfExitToRunExitMatch {
                                source_file: sf,
                                location: run_of_exit_location(
                                    sf,
                                    call.expression(),
                                    runner_name_node,
                                ),
                                exit_transformation,
                                runner_callee,
                                runner_name,
                                replacement_name,
                            },
                        );
                    }
                }
            }
        }
        node.for_each_child(&mut |child| {
            walk(tp, sf, child, matches);
            false
        });
    }
    walk(tp, sf, sf.as_node(), &mut matches);

    // Rust: Go's `sort.Slice` is unstable. The locations are distinct ranges (`appendRunOfExitMatch`), and two
    // distinct error ranges of these callees never start at the same position in practice; a stable sort is used.
    matches.sort_by_key(|m| m.location.pos());
    matches
}

// go: internal/rules/run_of_exit_to_run_exit.go:136 appendRunOfExitMatch
//
// Rust: appends in place (Go returns the grown slice).
pub(crate) fn append_run_of_exit_match(
    matches: &mut Vec<RunOfExitToRunExitMatch>,
    candidate: RunOfExitToRunExitMatch,
) {
    for m in matches.iter() {
        if m.location.pos() == candidate.location.pos()
            && m.location.end() == candidate.location.end()
        {
            return;
        }
    }
    matches.push(candidate);
}

// go: internal/rules/run_of_exit_to_run_exit.go:145 runOfExitLocation
pub(crate) fn run_of_exit_location(
    sf: P<SourceFile>,
    callee: P<Node>,
    name_node: Option<P<Node>>,
) -> TextRange {
    let mut location_node = callee;
    if let Some(parent) = name_node.and_then(|name_node| name_node.parent()) {
        location_node = parent;
    }
    ets_shim::scanner::get_error_range_for_node(sf, location_node)
}

// go: internal/rules/run_of_exit_to_run_exit.go:153 runOfExitRunner
//
// Rust: Go's named results `(targetNode, nameNode, runnerName, replacementName)`.
pub(crate) fn run_of_exit_runner(
    tp: &mut TypeParser<'_>,
    callee: Option<P<Node>>,
) -> (Option<P<Node>>, Option<P<Node>>, &'static str, &'static str) {
    let Some(callee) = callee else {
        return (None, None, "", "");
    };

    let mut target = callee;
    if callee.kind() == Kind::CallExpression {
        target = callee.as_call_expression().expression();
    }

    for (runner, replacement) in
        [("runPromise", "runPromiseExit"), ("runPromiseWith", "runPromiseExitWith")]
    {
        if !tp.is_node_reference_to_effect_module_api(target, runner) {
            continue;
        }
        let mut name_node = None;
        if target.kind() == Kind::PropertyAccessExpression {
            name_node = Some(target.as_property_access_expression().name());
        }
        return (Some(target), name_node, runner, replacement);
    }

    (None, None, "", "")
}
