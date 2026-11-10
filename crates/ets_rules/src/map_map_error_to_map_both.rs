use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{PipingFlow, PipingFlowTransformation, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_0_EFFECT_1_TRANSFORM_THE_SUCCESS_AND_FAILURE_CHANNELS_ONE_STEP_AT_A_TIME_EFFECT_2_EXPRESSES_BOTH_TRANSFORMATIONS_IN_A_SINGLE_STEP_EFFECT_MAP_MAP_ERROR_TO_MAP_BOTH;

// go: internal/rules/map_map_error_to_map_both.go:16 MapMapErrorToMapBoth ae1a13250929ac22
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

// go: internal/rules/map_map_error_to_map_both.go:37 MapMapErrorToMapBothMatch 7febd2044dbf13d5
#[derive(Clone)]
pub struct MapMapErrorToMapBothMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub first_name: &'static str,
    pub second_name: &'static str,
    pub replacement_name: &'static str,
    // FirstFailure is true when the first step transforms the failure channel.
    pub first_failure: bool,
    // Eager is true when both steps are the eager variants.
    pub eager: bool,
    // First and Second are the two steps in flow order.
    pub first: PipingFlowTransformation,
    pub second: PipingFlowTransformation,
}

// go: internal/rules/map_map_error_to_map_both.go:50 mapMapErrorStep b9eff7a0d0d73dbd
#[derive(Clone, Copy)]
pub(crate) struct MapMapErrorStep {
    name: &'static str,
    failure: bool,
    eager: bool,
}

// go: internal/rules/map_map_error_to_map_both.go:56 mapMapErrorSteps 7450978eb2dbee50
static MAP_MAP_ERROR_STEPS: [MapMapErrorStep; 4] = [
    MapMapErrorStep { name: "map", failure: false, eager: false },
    MapMapErrorStep { name: "mapError", failure: true, eager: false },
    MapMapErrorStep { name: "mapEager", failure: false, eager: true },
    MapMapErrorStep { name: "mapErrorEager", failure: true, eager: true },
];

// go: internal/rules/map_map_error_to_map_both.go:65 AnalyzeMapMapErrorToMapBoth 1bdee2f322651939
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
                location: ets_shim::scanner::get_error_range_for_node(sf, second.callee()),
                first_name: a.name,
                second_name: b.name,
                replacement_name: replacement,
                first_failure: a.failure,
                eager: a.eager,
                first,
                second,
            });
        }
    }
    matches
}

// go: internal/rules/map_map_error_to_map_both.go:111 classifyMapMapErrorStep 1742c20e554ea552
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
