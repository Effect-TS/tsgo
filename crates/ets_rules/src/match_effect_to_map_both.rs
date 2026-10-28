use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::TypeParser;

use crate::*;

// go: internal/rules/match_effect_to_map_both.go:16 MatchEffectToMapBoth 36fadae40cbe1a1c
// MatchEffectToMapBoth suggests mapBoth when matchEffect only maps the error
// and success channels through Effect.fail and Effect.succeed, respectively.
pub static MATCH_EFFECT_TO_MAP_BOTH: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "matchEffectToMapBoth",
    group: "style",
    description: "Suggests Effect.mapBoth when Effect.matchEffect only transforms the failure and success channels",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        tsdiag::EFFECT_MAP_BOTH_EXPRESSES_THESE_FAILURE_AND_SUCCESS_TRANSFORMATIONS_MORE_DIRECTLY_THAN_EFFECT_MATCH_EFFECT_EFFECT_MATCH_EFFECT_TO_MAP_BOTH.code(),
    ],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let matches = analyze_match_effect_to_map_both(ctx.type_parser, Some(ctx.source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                &tsdiag::EFFECT_MAP_BOTH_EXPRESSES_THESE_FAILURE_AND_SUCCESS_TRANSFORMATIONS_MORE_DIRECTLY_THAN_EFFECT_MATCH_EFFECT_EFFECT_MATCH_EFFECT_TO_MAP_BOTH,
                &[],
                &[],
            ));
        }
        diagnostics
    },
}
});

// go: internal/rules/match_effect_to_map_both.go:37 MatchEffectToMapBothMatch fd90f1bf13493bc6
#[derive(Clone, Copy)]
pub struct MatchEffectToMapBothMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub callee_node: P<Node>,
    pub handler_results: [P<Node>; 2],
    pub constructor_args: [P<Node>; 2],
}

// go: internal/rules/match_effect_to_map_both.go:45 AnalyzeMatchEffectToMapBoth 988cd8ca9f069abc
//
// Rust: Go's unused checker argument is dropped (the parser holds the checker); Go's nil parser check is the type
// system's.
pub fn analyze_match_effect_to_map_both(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<MatchEffectToMapBothMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index);
            if transformation.args().len() != 1
                || !tp
                    .is_node_reference_to_effect_module_api(transformation.callee(), "matchEffect")
            {
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
            let Some((failure_result, failure_argument)) =
                match_effect_constructor_handler(tp, on_failure, "fail")
            else {
                continue;
            };
            let Some((success_result, success_argument)) =
                match_effect_constructor_handler(tp, on_success, "succeed")
            else {
                continue;
            };
            matches.push(MatchEffectToMapBothMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
                callee_node: transformation.callee(),
                handler_results: [failure_result, success_result],
                constructor_args: [failure_argument, success_argument],
            });
        }
    }
    matches
}
