use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{PipingFlowTransformationReplacement, Tracker};
use ets_rules::{SleepThenEffectToDelayForm, SleepThenEffectToDelayMatch};
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, NodeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::TransformationKind;

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_SLEEP_SEQUENCED_BEFORE_THIS_EFFECT_RE_IMPLEMENTS_EFFECT_DELAY_EFFECT_DELAY_EXPRESSES_THE_POSTPONED_EFFECT_DIRECTLY_EFFECT_SLEEP_THEN_EFFECT_TO_DELAY;

/// Rust-only: Go's `func(*rewriter.Tracker) *ast.Node` builders returned by `effectModuleMemberCallee`.
pub(crate) type ModuleMemberCallee = Box<dyn Fn(&Tracker) -> P<Node>>;

// go: internal/fixables/sleep_then_effect_to_delay.go:13 SleepThenEffectToDelayFix 9ae4d20f29c65ad8
pub static SLEEP_THEN_EFFECT_TO_DELAY_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "sleepThenEffectToDelay",
    description: "Rewrite to Effect.delay",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["sleepThenEffectToDelay_fix"],
    run: run_sleep_then_effect_to_delay_fix,
});

// go: internal/fixables/sleep_then_effect_to_delay.go:23 runSleepThenEffectToDelayFix 8de220bd92cdf3e7
fn run_sleep_then_effect_to_delay_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_sleep_then_effect_to_delay(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if !m.fixable {
            continue;
        }
        let Some(delay) = effect_module_member_callee(ctx, m.sequencer.callee(), "delay") else {
            continue;
        };
        let mut suspend = None;
        if m.form != SleepThenEffectToDelayForm::EffectValue
            && m.form != SleepThenEffectToDelayForm::InlineResult
        {
            suspend = effect_module_member_callee(ctx, m.sequencer.callee(), "suspend");
            if suspend.is_none() {
                continue;
            }
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Rewrite to Effect.delay".to_string(),
            run: Box::new(move |tracker| {
                let (mut delayed, original) =
                    sleep_then_effect_to_delay_effect(tracker, &m, suspend.as_ref());
                match m.sequencer.kind() {
                    TransformationKind::DataFirst
                    | TransformationKind::DataLast
                    | TransformationKind::Call => {
                        if original.is_some() {
                            delayed = tracker.deep_clone_node(original);
                        }
                        let replacement = tracker.new_call_expression(
                            delay(tracker),
                            None,
                            None,
                            tracker.new_node_list(vec![
                                delayed.unwrap(),
                                tracker.deep_clone_node(Some(m.duration)).unwrap(),
                            ]),
                            NodeFlags::None,
                        );
                        tracker.replace_piping_flow_prefix(
                            Some(sf),
                            Some(&m.flow),
                            m.sleep_index + 2,
                            Some(replacement),
                        );
                    }
                    TransformationKind::Pipe | TransformationKind::Pipeable => {
                        // The delayed effect takes the place of the sleep call, and the
                        // sequencer stage becomes Effect.delay(duration).
                        if let Some(original) = original {
                            let sleep_call = m
                                .flow
                                .transformations()
                                .at(m.sleep_index)
                                .callee()
                                .parent()
                                .expect("a sleep call's callee has a parent");
                            delayed = subject_in_call_position(tracker, sf, sleep_call, original);
                            if delayed.is_none() {
                                return;
                            }
                        }
                        tracker.replace_piping_flow_prefix(
                            Some(sf),
                            Some(&m.flow),
                            m.sleep_index + 1,
                            delayed,
                        );
                        let callee = delay(tracker);
                        let arguments = tracker.new_node_list(vec![
                            tracker.deep_clone_node(Some(m.duration)).unwrap(),
                        ]);
                        tracker.replace_piping_flow_transformation(
                            Some(sf),
                            Some(&m.sequencer),
                            PipingFlowTransformationReplacement {
                                callee: Some(callee),
                                type_arguments: None,
                                arguments: Some(arguments),
                            },
                        );
                    }
                    _ => {}
                }
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}

// go: internal/fixables/sleep_then_effect_to_delay.go:82 sleepThenEffectToDelayEffect 30d61924dd34aa4d
// sleepThenEffectToDelayEffect returns the effect to delay: either a source
// expression that callers clone for its new position, or a synthesized
// Effect.suspend call that keeps the continuation lazy.
fn sleep_then_effect_to_delay_effect(
    tracker: &Tracker,
    m: &SleepThenEffectToDelayMatch,
    suspend: Option<&ModuleMemberCallee>,
) -> (Option<P<Node>>, Option<P<Node>>) {
    let lazy = match m.form {
        SleepThenEffectToDelayForm::EffectValue => return (None, Some(m.continuation)),
        SleepThenEffectToDelayForm::InlineResult => {
            return (None, m.lazy.map(|lazy| lazy.expression()));
        }
        SleepThenEffectToDelayForm::SuspendThunkDropParam => {
            // Effect.suspend takes a LazyArg, which a one-parameter arrow is not.
            let arrow_node = ast::skip_parentheses(m.continuation);
            let arrow = arrow_node.as_arrow_function();
            let mut return_type = None;
            if let Some(type_node) = arrow.type_() {
                return_type = tracker.deep_clone_node(Some(type_node));
            }
            tracker.new_arrow_function(
                None,
                None,
                Some(tracker.new_node_list(Vec::new())),
                return_type,
                None,
                Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
                tracker.deep_clone_node(arrow_node.body()),
            )
        }
        _ => tracker.deep_clone_node(Some(m.continuation)).unwrap(),
    };
    let suspend = suspend.expect("Effect.suspend is resolved for the suspend forms");
    (
        Some(tracker.new_call_expression(
            suspend(tracker),
            None,
            None,
            tracker.new_node_list(vec![lazy]),
            NodeFlags::None,
        )),
        None,
    )
}

// go: internal/fixables/sleep_then_effect_to_delay.go:108 effectModuleMemberCallee af959678a48e7369
// effectModuleMemberCallee returns a builder for a reference to the Effect
// module member name at location. The qualifier of the callee at location is
// kept when it is the Effect module; otherwise the file's Effect import, or an
// import binding in scope, is used. It returns nil when nothing reaches name.
fn effect_module_member_callee(
    ctx: &mut Context<'_, '_>,
    callee: P<Node>,
    name: &'static str,
) -> Option<ModuleMemberCallee> {
    let member = effect_module_member(name);
    if ast::is_property_access_expression(callee) {
        let expression = callee.as_property_access_expression().expression();
        if ctx.type_parser.is_expression_effect_module(expression) {
            let expression_type = ctx.type_parser.get_type_at_location(expression);
            if type_has_member(ctx, expression_type, &member) {
                return Some(Box::new(move |tracker: &Tracker| {
                    tracker.new_property_access_expression(
                        tracker.deep_clone_node(Some(expression)).unwrap(),
                        None,
                        tracker.new_identifier(name),
                        NodeFlags::None,
                    )
                }));
            }
        }
    }
    let reference = resolve_module_member(
        ctx,
        callee,
        &ets_typeparser::find_effect_module_reference(Some(ctx.source_file)),
        &member,
    )?;
    Some(Box::new(move |tracker: &Tracker| reference.expression(tracker, &member)))
}
