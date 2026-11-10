use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{PipingFlow, PipingFlowSubject, PipingFlowTransformation, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::EFFECT_0_EXPRESSES_THIS_OPTION_SUCCESS_VALUE_DIRECTLY_EFFECT_PREFER_SUCCEED_SOME_OR_NONE;

// go: internal/rules/prefer_succeed_some_or_none.go:16 PreferSucceedSomeOrNone 37029b0abad2842d
// PreferSucceedSomeOrNone suggests Effect.succeedNone and Effect.succeedSome for
// Effect.succeed calls that directly wrap Option.none or Option.some.
pub static PREFER_SUCCEED_SOME_OR_NONE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "preferSucceedSomeOrNone",
    group: "style",
    description: "Suggests using Effect.succeedNone or Effect.succeedSome instead of wrapping Option.none or Option.some with Effect.succeed",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_prefer_succeed_some_or_none(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.replacement_name],
            ));
        }
        diagnostics
    },
});

// go: internal/rules/prefer_succeed_some_or_none.go:42 PreferSucceedSomeOrNoneMatch 95d6588ace5ce94f
// PreferSucceedSomeOrNoneMatch holds the nodes needed by the diagnostic and quick fix.
//
// Rust: Go's `*PipingFlow` is an owned copy.
#[derive(Clone)]
pub struct PreferSucceedSomeOrNoneMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    // Flow's first TransformationCount steps end in Effect.succeed; for
    // succeedSome the step before it is Option.some.
    pub flow: Option<PipingFlow>,
    pub transformation_count: i64,
    pub replacement_name: &'static str, // succeedNone or succeedSome
}

// go: internal/rules/prefer_succeed_some_or_none.go:55 AnalyzePreferSucceedSomeOrNone f2bcfdbe0cf1dde0
// AnalyzePreferSucceedSomeOrNone finds piping flows in which Option.none or
// Option.some feeds directly into Effect.succeed. This covers both nested calls
// and pipe forms such as Option.none().pipe(Effect.succeed).
//
// Rust: Go's checker argument is the parser's checker; Go ignores it, so it is dropped. Go's nil parser check has
// no counterpart (the parser is a reference).
pub fn analyze_prefer_succeed_some_or_none(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<PreferSucceedSomeOrNoneMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        let is_succeed =
            |tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| -> bool {
                transformation.args().len() == 0
                    && transformation.type_arguments().is_none_or(|args| args.nodes().is_empty())
                    && tp.is_node_reference_to_effect_module_api(transformation.callee(), "succeed")
            };
        if PipingFlow::matches_prefix(
            Some(flow),
            tp,
            &|tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
                is_option_none_call(tp, subject.node())
            },
            &[&is_succeed],
        ) {
            matches.push(prefer_succeed_some_or_none_match(sf, flow, 0, "succeedNone"));
        }

        let sequences = PipingFlow::find_transformation_sequences(
            Some(flow),
            tp,
            &[
                &|tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
                    transformation.args().len() == 0
                        && tp.is_node_reference_to_effect_option_module_api(
                            transformation.callee(),
                            "some",
                        )
                },
                &is_succeed,
            ],
        );
        for sequence in sequences.iter().flatten() {
            matches.push(prefer_succeed_some_or_none_match(
                sf,
                flow,
                sequence.start() + 1,
                "succeedSome",
            ));
        }
    }
    matches
}

// go: internal/rules/prefer_succeed_some_or_none.go:89 preferSucceedSomeOrNoneMatch cd2d011ac1e5cc84
pub(crate) fn prefer_succeed_some_or_none_match(
    sf: P<SourceFile>,
    flow: &PipingFlow,
    succeed_index: i64,
    replacement_name: &'static str,
) -> PreferSucceedSomeOrNoneMatch {
    PreferSucceedSomeOrNoneMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(
            sf,
            flow.transformations().at(succeed_index).callee(),
        ),
        flow: Some(flow.clone()),
        transformation_count: succeed_index + 1,
        replacement_name,
    }
}

// go: internal/rules/prefer_succeed_some_or_none.go:99 isOptionNoneCall 44b554b98187b299
pub(crate) fn is_option_none_call(tp: &mut TypeParser<'_>, node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if node.kind() != Kind::CallExpression {
        return false;
    }
    let call = node.as_call_expression();
    call.arguments().nodes().is_empty()
        && call.type_arguments().is_none_or(|args| args.nodes().is_empty())
        && tp.is_node_reference_to_effect_option_module_api(call.expression(), "none")
}
