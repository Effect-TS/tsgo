use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_readonly::Slice;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, PartialPipingFlow, PipingFlow, PipingFlowTransformation, TypeParser,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::THIS_EFFECT_FIRST_COMPLETION_RACE_HAS_EXACTLY_ONE_SLEEP_OR_DELAY_BASED_TIMER_ARM_EFFECT_TIMEOUT_OR_ELSE_EXPRESSES_THE_TIMEOUT_AND_FALLBACK_DIRECTLY_EFFECT_RACE_FIRST_WITH_SLEEP_TO_TIMEOUT;

// go: internal/rules/race_first_with_sleep_to_timeout.go:17 RaceFirstWithSleepToTimeout 86744cee55d823c8
// RaceFirstWithSleepToTimeout suggests timeoutOrElse when one side of an
// Effect first-completion race is a hand-rolled timer arm.
pub static RACE_FIRST_WITH_SLEEP_TO_TIMEOUT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "raceFirstWithSleepToTimeout",
    group: "style",
    description: "Suggests Effect.timeoutOrElse when Effect.raceFirst has exactly one sleep- or delay-based timer arm",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_race_first_with_sleep_to_timeout(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/race_first_with_sleep_to_timeout.go:41 RaceFirstWithSleepToTimeoutMatch 118670b6a45a64f7
#[derive(Clone)]
pub struct RaceFirstWithSleepToTimeoutMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
}

// go: internal/rules/race_first_with_sleep_to_timeout.go:48 AnalyzeRaceFirstWithSleepToTimeout 0bdd7f3eaeeb4520
// AnalyzeRaceFirstWithSleepToTimeout finds Effect.raceFirst calls, and
// two-element literal Effect.raceAllFirst calls, with exactly one timer arm.
//
// Rust: Go's checker argument is the parser's checker (never nil here), so it is dropped.
pub fn analyze_race_first_with_sleep_to_timeout(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<RaceFirstWithSleepToTimeoutMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        for (i, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(i);

            let left: Option<PartialPipingFlow>;
            let mut right: Option<PartialPipingFlow> = None;
            if tp.is_node_reference_to_effect_module_api(transformation.callee(), "raceFirst") {
                // An onWinner option has observable behavior that timeoutOrElse
                // cannot carry over, so only match the two-arm form.
                if transformation.args().len() != 1 {
                    continue;
                }
                left = PipingFlow::copy_prefix(Some(flow), i);
                if let Some(right_flow) =
                    tp.longest_piping_flow_at(*transformation.args().at(0), true)
                {
                    right = Some(right_flow.partial());
                }
            } else if tp
                .is_node_reference_to_effect_module_api(transformation.callee(), "raceAllFirst")
            {
                let input = PipingFlow::copy_prefix(Some(flow), i);
                let Some(subject_node) = input.as_ref().and_then(|input| input.subject().node())
                else {
                    continue;
                };
                let array = ast::skip_parentheses(subject_node);
                if array.kind() != Kind::ArrayLiteralExpression
                    || array.as_array_literal_expression().elements().nodes().len() != 2
                {
                    continue;
                }
                let elements = array.as_array_literal_expression().elements().nodes();
                left = tp
                    .longest_piping_flow_at(elements[0], true)
                    .map(|left_flow| left_flow.partial());
                if let Some(right_flow) = tp.longest_piping_flow_at(elements[1], true) {
                    right = Some(right_flow.partial());
                }
            } else {
                continue;
            }

            if is_race_first_timer_flow(tp, left.as_ref())
                == is_race_first_timer_flow(tp, right.as_ref())
            {
                continue;
            }
            matches.push(RaceFirstWithSleepToTimeoutMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
            });
        }
    }
    matches
}

// go: internal/rules/race_first_with_sleep_to_timeout.go:109 isRaceFirstTimerFlow 7adc1b90056ae470
pub(crate) fn is_race_first_timer_flow(
    tp: &mut TypeParser<'_>,
    flow: Option<&PartialPipingFlow>,
) -> bool {
    let Some(flow) = flow else {
        return false;
    };
    if flow.transformations().len() == 0 {
        return false;
    }

    for (i, _) in flow.transformations().all() {
        let transformation = flow.transformations().at(i);
        if tp.is_node_reference_to_effect_module_api(transformation.callee(), "sleep") {
            return i == 0
                && timer_trailing_transformations(
                    tp,
                    flow.transformations().slice(i + 1, flow.transformations().len()),
                );
        }
        if tp.is_node_reference_to_effect_module_api(transformation.callee(), "delay") {
            return timer_trailing_transformations(
                tp,
                flow.transformations().slice(i + 1, flow.transformations().len()),
            );
        }
    }
    false
}

// go: internal/rules/race_first_with_sleep_to_timeout.go:129 timerTrailingTransformations ca2a0738ea4d10d2
pub(crate) fn timer_trailing_transformations(
    tp: &mut TypeParser<'_>,
    transformations: Slice<'_, PipingFlowTransformation>,
) -> bool {
    for (_, transformation) in transformations.all() {
        let callee = transformation.callee();
        if !tp.is_node_reference_to_effect_module_api(callee, "flatMap")
            && !tp.is_node_reference_to_effect_module_api(callee, "andThen")
            && !tp.is_node_reference_to_effect_module_api(callee, "as")
            && !tp.is_node_reference_to_effect_module_api(callee, "map")
        {
            return false;
        }
    }
    true
}
