use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, SourceFile};
use ets_shim::checker::{ElementFlags, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectContextFlags, EffectMajorVersion, PipingFlow, TypeParser};

/// Rust-only: the rule's message with a replacement (Go spells it out at each use).
static MESSAGE_WITH_REPLACEMENT: &Message = &tsdiag::THIS_EFFECT_CODE_RACES_PROMISES_WITH_PROMISE_0_THE_LOSING_PROMISES_KEEP_RUNNING_AND_ARE_NOT_INTERRUPTED_USE_EFFECT_1_OVER_EFFECT_VALUES_INSTEAD_EFFECT_PROMISE_RACE_IN_EFFECT;

/// Rust-only: the rule's message without a replacement (Go spells it out at each use).
static MESSAGE_WITHOUT_REPLACEMENT: &Message = &tsdiag::THIS_EFFECT_CODE_RACES_PROMISES_WITH_PROMISE_RACE_THE_LOSING_PROMISES_KEEP_RUNNING_AND_ARE_NOT_INTERRUPTED_EFFECT_PROMISE_RACE_IN_EFFECT;

// go: internal/rules/promise_race_in_effect.go:14 PromiseRaceInEffect 9a04d18f84d34361
pub static PROMISE_RACE_IN_EFFECT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "promiseRaceInEffect",
    group: "effectNative",
    description: "Warns when Promise.race or Promise.any races a list of promises inside Effect code, leaving the losing promises running",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE_WITH_REPLACEMENT.code(), MESSAGE_WITHOUT_REPLACEMENT.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let matches = analyze_promise_race_in_effect(ctx.type_parser, ctx.source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            if m.replacement.is_empty() {
                diags.push(ctx.new_diagnostic(
                    m.source_file,
                    m.location,
                    MESSAGE_WITHOUT_REPLACEMENT,
                    &[],
                    &[],
                ));
                continue;
            }
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE_WITH_REPLACEMENT,
                &[],
                &[m.method, m.replacement],
            ));
        }
        diags
    },
});

// go: internal/rules/promise_race_in_effect.go:52 PromiseRaceInEffectMatch 62c07c809d4dcf0a
// PromiseRaceInEffectMatch is a global Promise.race or Promise.any step that
// races a list of promises inside Effect code.
//
// Rust: `Method` and `Replacement` are each one of a few literals, so they are `&'static str`.
#[derive(Clone)]
pub struct PromiseRaceInEffectMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    // Method is "race" or "any".
    pub method: &'static str,
    // Replacement is the Effect combinator named by the message: "raceAll" for
    // any, "raceAllFirst" for race on Effect v4, and "" for race on Effect v3,
    // which has no list version of raceFirst.
    pub replacement: &'static str,
    pub flow: PipingFlow,
    pub index: i64,
}

// go: internal/rules/promise_race_in_effect.go:68 AnalyzePromiseRaceInEffect 8e0b960ae87b099c
// AnalyzePromiseRaceInEffect returns the Promise.race and Promise.any steps
// that run inside Effect code (an Effect constructor thunk, or an Effect.gen or
// generator Effect.fn body) over an array or a tuple with a rest element.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`). A transformation's callee is never nil, so
// Go's nil check on it drops.
pub fn analyze_promise_race_in_effect(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<PromiseRaceInEffectMatch> {
    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (index, transformation) in flow.transformations().all() {
            let callee = transformation.callee();
            if transformation.args().len() != 0
                || !tp.get_effect_context_flags(callee).intersects(EffectContextFlags::InEffect)
            {
                continue;
            }
            let mut method = "";
            for candidate in ["race", "any"] {
                if tp.is_node_reference_to_global_member(callee, "Promise", candidate) {
                    method = candidate;
                    break;
                }
            }
            if method.is_empty() || {
                let input_type = PipingFlow::transformation_input_type(Some(flow), tp, index);
                !promise_race_in_effect_is_variable_length_list(tp, input_type)
            } {
                continue;
            }
            let mut replacement = "raceAll";
            if method == "race" {
                replacement = "";
                if tp.supported_effect_version() == EffectMajorVersion::V4 {
                    replacement = "raceAllFirst";
                }
            }
            matches.push(PromiseRaceInEffectMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                method,
                replacement,
                flow: flow.clone(),
                index,
            });
        }
    }
    matches
}

// go: internal/rules/promise_race_in_effect.go:107 promiseRaceInEffectIsVariableLengthList ecaaf17f8cfd5ae0
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn promise_race_in_effect_is_variable_length_list(
    tp: &mut TypeParser<'_>,
    t: Option<P<Type>>,
) -> bool {
    let members = promise_race_in_effect_constraint_members(tp, t);
    if members.is_empty() {
        return false;
    }
    for member in members {
        let constrained_members = promise_race_in_effect_constraint_members(tp, Some(member));
        if constrained_members.is_empty() {
            return false;
        }
        for constrained in constrained_members {
            if constrained.flags().intersects(TypeFlags::Instantiable) {
                return false;
            }
            if tp.array_type(constrained).is_none()
                && (!ets_shim::checker::is_tuple_type_exported(constrained)
                    || !promise_race_in_effect_has_rest(constrained))
            {
                return false;
            }
        }
    }
    true
}

// go: internal/rules/promise_race_in_effect.go:129 promiseRaceInEffectConstraintMembers 5c79e7b118da923c
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); Go's nil result is an empty `Vec`.
pub(crate) fn promise_race_in_effect_constraint_members(
    tp: &mut TypeParser<'_>,
    mut t: Option<P<Type>>,
) -> Vec<P<Type>> {
    if let Some(ty) = t
        && ty.flags().intersects(TypeFlags::Instantiable)
    {
        t = tp.checker().get_base_constraint_of_type(ty);
    }
    tp.unroll_union_members(t).unwrap_or_default()
}

// go: internal/rules/promise_race_in_effect.go:136 promiseRaceInEffectHasRest 007d2c8a52ab6de2
//
// Rust: tsrs's `Type::target_tuple_type` is `pub(crate)`; a tuple type reference's `target()` is the same field,
// so Go's `TargetTupleType()` is `target().as_tuple_type()` (see `TSRS_API.md`).
pub(crate) fn promise_race_in_effect_has_rest(tuple: P<Type>) -> bool {
    let target = tuple.target().expect("a tuple type reference has a target");
    for flags in target.as_tuple_type().element_flags() {
        if flags.intersects(ElementFlags::Variable) {
            return true;
        }
    }
    false
}
