use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, NodeList, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    PipingFlow, PipingFlowSubject, PipingFlowTransformation, TransformationKind, TypeParser,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::EFFECT_0_EXPRESSES_THIS_OPTION_SUCCESS_VALUE_DIRECTLY_EFFECT_PREFER_SUCCEED_SOME_OR_NONE;

// go: internal/rules/prefer_succeed_some_or_none.go:16 PreferSucceedSomeOrNone
// PreferSucceedSomeOrNone suggests Effect.succeedNone and Effect.succeedSome for
// Effect.succeed calls that directly wrap Option.none or Option.some.
pub static PREFER_SUCCEED_SOME_OR_NONE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "preferSucceedSomeOrNone",
    group: "style",
    description: "Suggests using Effect.succeedNone or Effect.succeedSome instead of wrapping Option.none or Option.some with Effect.succeed",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
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

// go: internal/rules/prefer_succeed_some_or_none.go:42 PreferSucceedSomeOrNoneMatch
// PreferSucceedSomeOrNoneMatch holds the nodes needed by the diagnostic and quick fix.
//
// Rust: Go's `*PipingFlow` (nil unless the succeed step is a call, pipe or pipeable step) is an owned copy.
#[derive(Clone)]
pub struct PreferSucceedSomeOrNoneMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub flow: Option<PipingFlow>,
    pub transformation_count: i64,
    pub replacement_name: &'static str,
    pub value_node: Option<P<Node>>,
    pub value_type_arguments: Option<P<NodeList>>,
}

// go: internal/rules/prefer_succeed_some_or_none.go:52 normalizedOptionInput
pub(crate) struct NormalizedOptionInput {
    pub(crate) replacement_name: &'static str,
    pub(crate) value_node: Option<P<Node>>,
    pub(crate) value_type_arguments: Option<P<NodeList>>,
}

// go: internal/rules/prefer_succeed_some_or_none.go:61 AnalyzePreferSucceedSomeOrNone
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
            matches.push(prefer_succeed_some_or_none_match(
                sf,
                flow,
                0,
                &NormalizedOptionInput {
                    replacement_name: "succeedNone",
                    value_node: None,
                    value_type_arguments: None,
                },
            ));
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
            let option_index = sequence.start();
            matches.push(prefer_succeed_some_or_none_match(
                sf,
                flow,
                option_index + 1,
                &NormalizedOptionInput {
                    replacement_name: "succeedSome",
                    value_node: PipingFlow::transformation_input_node(Some(flow), option_index),
                    value_type_arguments: flow.transformations().at(option_index).type_arguments(),
                },
            ));
        }
    }
    matches
}

// go: internal/rules/prefer_succeed_some_or_none.go:100 preferSucceedSomeOrNoneMatch
pub(crate) fn prefer_succeed_some_or_none_match(
    sf: P<SourceFile>,
    flow: &PipingFlow,
    succeed_index: i64,
    option_input: &NormalizedOptionInput,
) -> PreferSucceedSomeOrNoneMatch {
    let transformation = flow.transformations().at(succeed_index);
    let mut m = PreferSucceedSomeOrNoneMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
        flow: None,
        transformation_count: 0,
        replacement_name: option_input.replacement_name,
        value_node: option_input.value_node,
        value_type_arguments: option_input.value_type_arguments,
    };
    if transformation.kind() == TransformationKind::Call
        || transformation.kind() == TransformationKind::Pipe
        || transformation.kind() == TransformationKind::Pipeable
    {
        m.flow = Some(flow.clone());
        m.transformation_count = succeed_index + 1;
    }
    m
}

// go: internal/rules/prefer_succeed_some_or_none.go:118 isOptionNoneCall
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
