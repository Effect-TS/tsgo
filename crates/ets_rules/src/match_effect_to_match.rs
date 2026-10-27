use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{LazyExpressionFlags, PipingFlow, TypeParser};

// go: internal/rules/match_effect_to_match.go:16 MatchEffectToMatch 1b72bdc3e494120a
// MatchEffectToMatch suggests the non-effectful match variant when both
// handlers only lift their result with Effect.succeed.
pub static MATCH_EFFECT_TO_MATCH: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "matchEffectToMatch",
    group: "style",
    description: "Suggests Effect.match or Effect.matchCause when both Effect.matchEffect handlers only return Effect.succeed",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        tsdiag::EFFECT_0_EXPRESSES_THIS_NON_EFFECTFUL_FOLD_MORE_DIRECTLY_THAN_EFFECT_1_WITH_EFFECT_SUCCEED_HANDLERS_EFFECT_MATCH_EFFECT_TO_MATCH.code(),
    ],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let matches = analyze_match_effect_to_match(ctx.type_parser, Some(ctx.source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                &tsdiag::EFFECT_0_EXPRESSES_THIS_NON_EFFECTFUL_FOLD_MORE_DIRECTLY_THAN_EFFECT_1_WITH_EFFECT_SUCCEED_HANDLERS_EFFECT_MATCH_EFFECT_TO_MATCH,
                &[],
                &[m.replacement_name, m.match_effect_name],
            ));
        }
        diagnostics
    },
}
});

// go: internal/rules/match_effect_to_match.go:37 MatchEffectToMatchMatch 317c0494aeeb9243
#[derive(Clone, Copy)]
pub struct MatchEffectToMatchMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub callee_node: P<Node>,
    pub match_effect_name: &'static str,
    pub replacement_name: &'static str,
    pub handler_results: [P<Node>; 2],
    pub succeed_arguments: [P<Node>; 2],
}

// go: internal/rules/match_effect_to_match.go:47 AnalyzeMatchEffectToMatch 9a7c3365c644c20e
//
// Rust: Go's unused checker argument is dropped (the parser holds the checker); Go's nil parser check is the type
// system's.
pub fn analyze_match_effect_to_match(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<MatchEffectToMatchMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index);
            if transformation.args().len() != 1 {
                continue;
            }
            let on_failure = ets_typeparser::object_literal_property_initializer(
                *transformation.args().at(0),
                "onFailure",
            );
            let on_success = ets_typeparser::object_literal_property_initializer(
                *transformation.args().at(0),
                "onSuccess",
            );
            let (Some(on_failure), Some(on_success)) = (on_failure, on_success) else {
                continue;
            };
            let (match_effect_name, replacement_name);
            if tp.is_node_reference_to_effect_module_api(transformation.callee(), "matchEffect") {
                (match_effect_name, replacement_name) = ("matchEffect", "match");
            } else if tp
                .is_node_reference_to_effect_module_api(transformation.callee(), "matchCauseEffect")
            {
                (match_effect_name, replacement_name) = ("matchCauseEffect", "matchCause");
            } else {
                continue;
            }

            let Some((failure_result, failure_argument)) =
                match_effect_constructor_handler(tp, on_failure, "succeed")
            else {
                continue;
            };
            let Some((success_result, success_argument)) =
                match_effect_constructor_handler(tp, on_success, "succeed")
            else {
                continue;
            };
            matches.push(MatchEffectToMatchMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
                callee_node: transformation.callee(),
                match_effect_name,
                replacement_name,
                handler_results: [failure_result, success_result],
                succeed_arguments: [failure_argument, success_argument],
            });
        }
    }
    matches
}

// go: internal/rules/match_effect_to_match.go:91 matchEffectConstructorHandler 5a87f87e6b4c80b2
//
// Rust: Go's `(result, argument, ok)` is `Option<(result, argument)>`.
pub(crate) fn match_effect_constructor_handler(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
    constructor_name: &str,
) -> Option<(P<Node>, P<Node>)> {
    let lazy = ets_typeparser::parse_lazy_expression(node, LazyExpressionFlags::None)?;
    let flow = tp.longest_piping_flow_at(lazy.expression(), false);
    let flow = flow.filter(|flow| flow.transformations().len() != 0)?;
    let last = flow.transformations().len() - 1;
    let transformation = flow.transformations().at(last);
    if !tp.is_node_reference_to_effect_module_api(transformation.callee(), constructor_name) {
        return None;
    }
    let argument = PipingFlow::transformation_input_node(Some(&flow), last)?;
    Some((lazy.expression(), argument))
}
