use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, ModifierFlags, Node, SourceFile};
use ets_shim::checker::{SignatureKind, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::scanner;
use ets_typeparser::{
    LazyExpressionFlags, ParsedLazyExpression, PipingFlow, PipingFlowTransformation,
    TransformationKind, TypeParser,
};

use crate::arrow_parameters::arrow_ignores_parameter;
use crate::piping_flow::has_explicit_type_arguments;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_SLEEP_SEQUENCED_BEFORE_THIS_EFFECT_RE_IMPLEMENTS_EFFECT_DELAY_EFFECT_DELAY_EXPRESSES_THE_POSTPONED_EFFECT_DIRECTLY_EFFECT_SLEEP_THEN_EFFECT_TO_DELAY;

// go: internal/rules/sleep_then_effect_to_delay.go:16 SleepThenEffectToDelay 35921b5d8be7d0e0
// SleepThenEffectToDelay suggests Effect.delay when Effect.sleep is sequenced
// in front of an effect, which is how Effect.delay itself is implemented.
pub static SLEEP_THEN_EFFECT_TO_DELAY: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "sleepThenEffectToDelay",
    group: "style",
    description: "Suggests Effect.delay instead of sequencing an effect after Effect.sleep",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_sleep_then_effect_to_delay(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/sleep_then_effect_to_delay.go:42 SleepThenEffectToDelayForm 09ee33301cf8ee98
// go: internal/rules/sleep_then_effect_to_delay.go:44 SleepThenEffectToDelayEffectValue 6b72a80b97c871b0
// SleepThenEffectToDelayForm describes how the continuation becomes the
// delayed effect.
//
// Rust: Go's `iota + 1` constants; the zero value ("not a continuation") is `None` where Go returns it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SleepThenEffectToDelayForm {
    // SleepThenEffectToDelayEffectValue: the continuation is the Effect itself.
    EffectValue = 1,
    // SleepThenEffectToDelayInlineResult: a thunk returns a value that is stable
    // at the sequencer, so the returned expression is used directly.
    InlineResult = 2,
    // SleepThenEffectToDelaySuspendThunk: a zero-parameter function builds the
    // Effect, so it is kept lazy with Effect.suspend.
    SuspendThunk = 3,
    // SleepThenEffectToDelaySuspendThunkDropParam: an arrow with one unused
    // parameter builds the Effect; Effect.suspend needs it without the parameter.
    SuspendThunkDropParam = 4,
    // SleepThenEffectToDelaySuspendReference: a reference to a zero-parameter
    // function returning an Effect.
    SuspendReference = 5,
}

// go: internal/rules/sleep_then_effect_to_delay.go:62 SleepThenEffectToDelayMatch 5a17cb7d9db50241
// SleepThenEffectToDelayMatch holds the nodes needed by the diagnostic and quick fix.
//
// Rust: Go's `*PipingFlow` is an owned copy.
#[derive(Clone)]
pub struct SleepThenEffectToDelayMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub flow: PipingFlow,
    pub sleep_index: i64,
    pub sequencer: PipingFlowTransformation,
    // Duration is the argument of the Effect.sleep call.
    pub duration: P<Node>,
    // Continuation is the sequencer argument as written.
    pub continuation: P<Node>,
    // Lazy is the parsed continuation for SleepThenEffectToDelayInlineResult.
    pub lazy: Option<ParsedLazyExpression>,
    pub form: SleepThenEffectToDelayForm,
    pub fixable: bool,
}

// go: internal/rules/sleep_then_effect_to_delay.go:82 AnalyzeSleepThenEffectToDelay a6b59cde73b1ad78
// AnalyzeSleepThenEffectToDelay finds piping flows in which a direct
// Effect.sleep call is immediately sequenced with Effect.andThen,
// Effect.flatMap or Effect.zipRight into a continuation whose result is an
// Effect that ignores the sleep's void result.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), so it is dropped. A transformation's
// callee is never nil in Rust, so those nil checks drop.
pub fn analyze_sleep_then_effect_to_delay(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<SleepThenEffectToDelayMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    let is_sleep = |tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| -> bool {
        step.kind() == TransformationKind::Call
            && step.args().len() == 0
            && tp.is_node_reference_to_effect_module_api(step.callee(), "sleep")
    };
    let is_sequencer = |tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| -> bool {
        // A single argument also rules out v3 zipRight options, which may run
        // both effects concurrently.
        if step.args().len() != 1 || step.args().at(0).kind() == Kind::SpreadElement {
            return false;
        }
        tp.is_node_reference_to_effect_module_api(step.callee(), "andThen")
            || tp.is_node_reference_to_effect_module_api(step.callee(), "flatMap")
            || tp.is_node_reference_to_effect_module_api(step.callee(), "zipRight")
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        let sequences =
            PipingFlow::find_transformation_sequences(Some(flow), tp, &[&is_sleep, &is_sequencer]);
        for sequence in sequences.iter().flatten() {
            let index = sequence.start();
            let sleep = sequence.transformations().at(0).clone();
            let sequencer = sequence.transformations().at(1).clone();
            let Some(duration) = PipingFlow::transformation_input_node(Some(flow), index) else {
                continue;
            };
            let continuation = *sequencer.args().at(0);
            let Some((form, lazy)) =
                classify_sleep_continuation(tp, continuation, sequencer.callee())
            else {
                continue;
            };
            let fixable = !has_explicit_type_arguments(Some(&sequencer))
                && sleep_then_effect_to_delay_keeps_invocation(tp, form, continuation)
                && sleep_then_effect_to_delay_keeps_order(
                    tp,
                    &sequencer,
                    form,
                    continuation,
                    duration,
                );
            matches.push(SleepThenEffectToDelayMatch {
                source_file: sf,
                location: scanner::get_error_range_for_node(sf, sleep.callee()),
                flow: flow.clone(),
                sleep_index: index,
                sequencer,
                duration,
                continuation,
                lazy,
                form,
                fixable,
            });
        }
    }
    matches
}

// go: internal/rules/sleep_then_effect_to_delay.go:141 sleepThenEffectToDelayKeepsOrder c1892a0e79d94daf
// sleepThenEffectToDelayKeepsOrder reports whether Effect.delay(k, d) keeps
// what the continuation and the duration evaluate to. Only a curried
// sequencer, `andThen(k)(sleep(d))`, already evaluates k first; every other
// form evaluates d, and sleep reads its value, before k. The rewrite is
// therefore safe only when evaluating k cannot change anything, as for
// function literals and stable values (the returned value of InlineResult is
// stable by construction), or when d is a stable primitive, whose value k
// cannot change. A stable binding of an object, such as a tuple, can still be
// mutated by k.
pub(crate) fn sleep_then_effect_to_delay_keeps_order(
    tp: &mut TypeParser<'_>,
    sequencer: &PipingFlowTransformation,
    form: SleepThenEffectToDelayForm,
    continuation: P<Node>,
    duration: P<Node>,
) -> bool {
    if sequencer.kind() == TransformationKind::Call {
        return true;
    }
    match form {
        SleepThenEffectToDelayForm::InlineResult
        | SleepThenEffectToDelayForm::SuspendThunk
        | SleepThenEffectToDelayForm::SuspendThunkDropParam => return true,
        _ => {}
    }
    let location = sequencer.callee();
    tp.is_expression_value_stable_at_location(continuation, location)
        || tp.is_expression_value_stable_at_location(duration, location)
            && is_primitive_typed(tp, duration)
}

// go: internal/rules/sleep_then_effect_to_delay.go:156 isPrimitiveTyped 94347179b094c64c
// isPrimitiveTyped reports whether every value of node's type is a string,
// number, bigint or boolean.
pub(crate) fn is_primitive_typed(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    let Some(t) = tp.get_type_at_location(node) else {
        return false;
    };
    for member in tp.unroll_union_members(t).unwrap_or_default() {
        if !member.flags().intersects(
            TypeFlags::StringLike
                | TypeFlags::NumberLike
                | TypeFlags::BigIntLike
                | TypeFlags::BooleanLike,
        ) {
            return false;
        }
    }
    true
}

// go: internal/rules/sleep_then_effect_to_delay.go:176 sleepThenEffectToDelayKeepsInvocation 254b47a26ac44156
// sleepThenEffectToDelayKeepsInvocation reports whether passing the
// continuation to Effect.suspend calls it as the sequencer did. The sequencer
// calls it with the sleep result, Effect.suspend with no arguments, which a
// non-arrow function can observe through `arguments` or `this` even without
// declared parameters. Inline arrows qualify, and so do references to a const
// binding initialized with an arrow; other references may be bound to any
// function.
pub(crate) fn sleep_then_effect_to_delay_keeps_invocation(
    tp: &mut TypeParser<'_>,
    form: SleepThenEffectToDelayForm,
    continuation: P<Node>,
) -> bool {
    match form {
        SleepThenEffectToDelayForm::SuspendThunk => {
            ast::skip_parentheses(continuation).kind() == Kind::ArrowFunction
        }
        SleepThenEffectToDelayForm::SuspendReference => {
            let reference = ast::skip_parentheses(continuation);
            if reference.kind() != Kind::Identifier {
                return false;
            }
            let Some(symbol) = tp.get_symbol_at_location(reference) else {
                return false;
            };
            if !tp.checker().is_constant_variable(symbol) {
                return false;
            }
            let declaration = symbol.value_declaration();
            declaration.is_some_and(|declaration| {
                ast::is_variable_declaration(declaration)
                    && declaration.initializer().is_some_and(|initializer| {
                        ast::skip_parentheses(initializer).kind() == Kind::ArrowFunction
                    })
            })
        }
        _ => true,
    }
}

// go: internal/rules/sleep_then_effect_to_delay.go:198 classifySleepContinuation 397cc6e2b3aff20d
// classifySleepContinuation returns how continuation becomes the delayed
// effect, or zero when it is not an Effect continuation that ignores its input.
//
// Rust: Go's zero form is `None`.
pub(crate) fn classify_sleep_continuation(
    tp: &mut TypeParser<'_>,
    continuation: P<Node>,
    location: P<Node>,
) -> Option<(SleepThenEffectToDelayForm, Option<ParsedLazyExpression>)> {
    let f = ast::skip_parentheses(continuation);
    match f.kind() {
        Kind::ArrowFunction | Kind::FunctionExpression => {
            if !continuation_function_ignores_input(tp, f) {
                return None;
            }
            let fn_type = tp.get_type_at_location(f);
            if !returns_single_strict_effect(tp, fn_type, false) {
                return None;
            }
            // Statements before the return run after the sleep, so only a lone
            // returned value can be inlined.
            if let Some(lazy) = ets_typeparser::parse_lazy_expression(f, LazyExpressionFlags::None)
                && tp.is_expression_value_stable_at_location(lazy.expression(), location)
            {
                return Some((SleepThenEffectToDelayForm::InlineResult, Some(lazy)));
            }
            if let Some(params) = ets_typeparser::get_function_like_parameters(f)
                && !params.nodes().is_empty()
            {
                return Some((SleepThenEffectToDelayForm::SuspendThunkDropParam, None));
            }
            return Some((SleepThenEffectToDelayForm::SuspendThunk, None));
        }
        _ => {}
    }
    if tp.strict_effect_type_at_location(continuation).is_some() {
        return Some((SleepThenEffectToDelayForm::EffectValue, None));
    }
    let continuation_type = tp.get_type_at_location(continuation);
    if returns_single_strict_effect(tp, continuation_type, true) {
        return Some((SleepThenEffectToDelayForm::SuspendReference, None));
    }
    None
}

// go: internal/rules/sleep_then_effect_to_delay.go:230 continuationFunctionIgnoresInput 980d92506ffbf964
// continuationFunctionIgnoresInput accepts arrows that never read their
// parameter and synchronous, non-generic function expressions without
// parameters. Such a function expression still receives the void sleep result
// in its arguments object, so sleepThenEffectToDelayKeepsInvocation withholds
// fixes that would call it differently.
pub(crate) fn continuation_function_ignores_input(tp: &mut TypeParser<'_>, f: P<Node>) -> bool {
    if f.kind() == Kind::ArrowFunction {
        return arrow_ignores_parameter(tp, Some(f));
    }
    let expression = f.as_function_expression();
    if expression.asterisk_token().is_some()
        || ast::get_combined_modifier_flags(f).intersects(ModifierFlags::Async)
    {
        return false;
    }
    if let Some(params) = ets_typeparser::get_function_like_type_parameters(f)
        && !params.nodes().is_empty()
    {
        return false;
    }
    let params = ets_typeparser::get_function_like_parameters(f);
    params.is_none_or(|params| params.nodes().is_empty())
}

// go: internal/rules/sleep_then_effect_to_delay.go:247 returnsSingleStrictEffect 8de9d5100d316d4b
// returnsSingleStrictEffect reports whether t has exactly one call signature,
// without parameters when requireNoParameters is set, that returns an Effect.
pub(crate) fn returns_single_strict_effect(
    tp: &mut TypeParser<'_>,
    t: Option<P<Type>>,
    require_no_parameters: bool,
) -> bool {
    let Some(t) = t else {
        return false;
    };
    let signatures = tp.checker().get_signatures_of_type_exported(t, SignatureKind::Call);
    if signatures.len() != 1 || require_no_parameters && !signatures[0].parameters().is_empty() {
        return false;
    }
    let return_type = tp.checker().get_return_type_of_signature(signatures[0]);
    tp.strict_effect_type(return_type).is_some()
}
