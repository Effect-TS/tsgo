use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{PipingFlowTransformationReplacement, Tracker};
use ets_rules::{ManualMetricTapToTrackKind, ManualMetricTapToTrackMatch};
use ets_shim::P;
use ets_shim::ast::{Kind, Node, NodeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message =
    &tsdiag::EFFECT_0_EXPRESSES_RECORDING_A_METRIC_ON_EVERY_1_DIRECTLY_EFFECT_MANUAL_METRIC_TAP_TO_TRACK;

// go: internal/fixables/manual_metric_tap_to_track.go:13 ManualMetricTapToTrackFix
pub static MANUAL_METRIC_TAP_TO_TRACK_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "manualMetricTapToTrack",
    description: "Replace Effect.tap with Effect.track*",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["manualMetricTapToTrack_fix"],
    run: run_manual_metric_tap_to_track_fix,
});

// go: internal/fixables/manual_metric_tap_to_track.go:23 runManualMetricTapToTrackFix
//
// Rust: the match's transformation is never nil (a non-optional field), so Go's nil check drops.
fn run_manual_metric_tap_to_track_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_manual_metric_tap_to_track(&mut ctx.type_parser, source_file) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        // Only offer the fix when every generated reference resolves at the call site.
        let callee = m.transformation.callee();
        let track = effect_module_member(m.track_api);
        let track_reference = resolve_module_member(
            ctx,
            callee,
            &ets_typeparser::find_effect_module_reference(Some(source_file)),
            &track,
        );
        let Some(track_reference) = track_reference else {
            continue;
        };
        let with_constant_input = metric_module_member("withConstantInput");
        let mut with_constant_input_reference = None;
        if m.kind == ManualMetricTapToTrackKind::ConstantInput {
            with_constant_input_reference = resolve_module_member(
                ctx,
                callee,
                &ets_typeparser::find_module_reference(Some(source_file), "Metric"),
                &with_constant_input,
            );
            if with_constant_input_reference.is_none() {
                continue;
            }
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: format!("Replace with Effect.{}", m.track_api),
            run: Box::new(move |tracker| {
                let callee = track_reference.expression(tracker, &track);
                let arguments =
                    tracker.new_node_list(manual_metric_track_arguments(tracker, &m, |tracker| {
                        with_constant_input_reference
                            .as_ref()
                            .expect("resolved for a constant input")
                            .expression(tracker, &with_constant_input)
                    }));
                tracker.replace_piping_flow_transformation(
                    Some(source_file),
                    Some(&m.transformation),
                    PipingFlowTransformationReplacement {
                        callee: Some(callee),
                        type_arguments: None,
                        arguments: Some(arguments),
                    },
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}

// go: internal/fixables/manual_metric_tap_to_track.go:65 effectModuleMember
//
// Rust: the predicates get the type parser as an argument, so Go's `ctx` parameter drops.
#[expect(
    clippy::redundant_closure_for_method_calls,
    reason = "the method path is not general over the parser's lifetime"
)]
pub(crate) fn effect_module_member(name: &'static str) -> ModuleMember {
    ModuleMember {
        name,
        is_module: Box::new(|tp, node| tp.is_expression_effect_module(node)),
        is_member: Box::new(move |tp, node| tp.is_node_reference_to_effect_module_api(node, name)),
    }
}

// go: internal/fixables/manual_metric_tap_to_track.go:73 metricModuleMember
//
// Rust: the predicates get the type parser as an argument, so Go's `ctx` parameter drops.
#[expect(
    clippy::redundant_closure_for_method_calls,
    reason = "the method path is not general over the parser's lifetime"
)]
fn metric_module_member(name: &'static str) -> ModuleMember {
    ModuleMember {
        name,
        is_module: Box::new(|tp, node| tp.is_expression_effect_metric_module(node)),
        is_member: Box::new(move |tp, node| {
            tp.is_node_reference_to_effect_metric_module_api(node, name)
        }),
    }
}

// go: internal/fixables/manual_metric_tap_to_track.go:81 manualMetricTrackArguments
//
// Rust: the callback gets the tracker as an argument (Go's closure captures it).
fn manual_metric_track_arguments(
    tracker: &Tracker,
    m: &ManualMetricTapToTrackMatch,
    with_constant_input: impl FnOnce(&Tracker) -> P<Node>,
) -> Vec<P<Node>> {
    let metric = tracker.deep_clone_node(Some(m.metric)).unwrap();
    match m.kind {
        ManualMetricTapToTrackKind::Mapper => {
            let mut params = Vec::with_capacity(m.params.len());
            for &param in &m.params {
                params.push(tracker.deep_clone_node(Some(param)).unwrap());
            }
            let mapper = tracker.new_arrow_function(
                None,
                None,
                Some(tracker.new_node_list(params)),
                None,
                None,
                Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
                tracker.deep_clone_node(Some(m.input)),
            );
            return vec![metric, mapper];
        }
        ManualMetricTapToTrackKind::ConstantInput => {
            let call = tracker.new_call_expression(
                with_constant_input(tracker),
                None,
                None,
                tracker
                    .new_node_list(vec![metric, tracker.deep_clone_node(Some(m.input)).unwrap()]),
                NodeFlags::None,
            );
            return vec![call];
        }
        ManualMetricTapToTrackKind::Verbatim => {}
    }
    vec![metric]
}
