use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, NodeList, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, LazyExpressionFlags, PipingFlow, PipingFlowSubject,
    PipingFlowTransformation, TransformationKind, TypeParser,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_0_EXPRESSES_THIS_FAILURE_DIRECTLY_INSTEAD_OF_WRAPPING_CAUSE_2_IN_EFFECT_1_EFFECT_FAIL_CAUSE_TO_CONSTRUCTOR;

// go: internal/rules/fail_cause_to_constructor.go:17 FailCauseToConstructor 647d57f58e521533
// FailCauseToConstructor suggests Effect.fail, Effect.die, Effect.failSync and
// Effect.dieSync when Effect.failCause or Effect.failCauseSync wraps a single
// Cause.fail or Cause.die.
pub static FAIL_CAUSE_TO_CONSTRUCTOR: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "failCauseToConstructor",
    group: "style",
    description: "Suggests Effect.fail/die/failSync/dieSync instead of failCause/failCauseSync wrapping a single Cause.fail/die",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_fail_cause_to_constructor(ctx.type_parser, Some(source_file));
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&m.replacement_name, m.source_name, m.constructor_name],
            ));
        }
        diags
    },
});

// go: internal/rules/fail_cause_to_constructor.go:45 FailCauseToConstructorMatch 7b1ae6cc70061555
// FailCauseToConstructorMatch holds the nodes needed by the diagnostic and quick fix.
//
// Rust: Go's `*PipingFlow` is an owned copy.
#[derive(Clone)]
pub struct FailCauseToConstructorMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub replacement_name: String,       // fail, die, failSync or dieSync
    pub source_name: &'static str,      // failCause or failCauseSync
    pub constructor_name: &'static str, // fail or die
    pub value_node: Option<P<Node>>,    // The value passed to Cause.fail / Cause.die
    pub value_type_arguments: Option<P<NodeList>>,
    // Eager fix: the flow prefix up to and including Effect.failCause is
    // replaced. A nil Flow means the match is reported without a fix.
    pub flow: Option<PipingFlow>,
    pub transformation_count: i64,
    // Lazy fix: the Cause constructor call in the thunk is replaced by its
    // value and the failCauseSync callee is renamed. A nil LazyCauseNode means
    // the match is reported without a fix.
    pub lazy_callee: Option<P<Node>>,
    pub lazy_cause_node: Option<P<Node>>,
    pub lazy_expression_body: bool,
}

// go: internal/rules/fail_cause_to_constructor.go:69 AnalyzeFailCauseToConstructor 6277e3462181d23c
// AnalyzeFailCauseToConstructor finds piping flows in which Cause.fail or
// Cause.die feeds directly into Effect.failCause, and Effect.failCauseSync
// thunks whose returned flow ends in Cause.fail (or Cause.die on Effect v3,
// which has Effect.dieSync).
//
// Rust: Go's checker argument is the parser's checker, so it is dropped; Go's nil parser check has no counterpart
// (the parser is a reference). Go's `Callee() != nil` and `lazy.Expression() == nil` checks drop, both are
// non-optional here.
pub fn analyze_fail_cause_to_constructor(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<FailCauseToConstructorMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let no_type_arguments = |transformation: &PipingFlowTransformation| -> bool {
        transformation.type_arguments().is_none_or(|arguments| arguments.nodes().is_empty())
    };
    let is_cause_constructor = |name: &'static str| {
        move |tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| -> bool {
            transformation.args().len() == 0
                && tp.is_node_reference_to_effect_cause_module_api(transformation.callee(), name)
        }
    };
    let is_effect_api = |name: &'static str| {
        move |tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| -> bool {
            transformation.args().len() == 0
                && no_type_arguments(transformation)
                && tp.is_node_reference_to_effect_module_api(transformation.callee(), name)
        }
    };
    let allow_lazy_die = tp.supported_effect_version() != EffectMajorVersion::V4;

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        for constructor_name in ["fail", "die"] {
            let sequences = PipingFlow::find_transformation_sequences(
                Some(flow),
                tp,
                &[&is_cause_constructor(constructor_name), &is_effect_api("failCause")],
            );
            for sequence in sequences.iter().flatten() {
                let cause_index = sequence.start();
                let cause_step = flow.transformations().at(cause_index);
                let fail_step = flow.transformations().at(cause_index + 1);
                let value = PipingFlow::transformation_input_node(Some(flow), cause_index);
                let mut m = FailCauseToConstructorMatch {
                    source_file: sf,
                    location: ets_shim::scanner::get_error_range_for_node(sf, fail_step.callee()),
                    replacement_name: constructor_name.to_string(),
                    source_name: "failCause",
                    constructor_name,
                    value_node: value,
                    value_type_arguments: cause_step.type_arguments(),
                    flow: None,
                    transformation_count: 0,
                    lazy_callee: None,
                    lazy_cause_node: None,
                    lazy_expression_body: false,
                };
                if value.is_some()
                    && (fail_step.kind() == TransformationKind::Call
                        || fail_step.kind() == TransformationKind::Pipe
                        || fail_step.kind() == TransformationKind::Pipeable)
                {
                    m.flow = Some(flow.clone());
                    m.transformation_count = cause_index + 2;
                }
                matches.push(m);
            }
        }

        // An arrow or function expression is always the subject of its flow,
        // so Effect.failCauseSync is the first transformation.
        let is_thunk_subject = |_: &mut TypeParser<'_>, subject: &PipingFlowSubject| -> bool {
            subject.node().is_some_and(|node| {
                node.kind() == Kind::ArrowFunction || node.kind() == Kind::FunctionExpression
            })
        };
        if !PipingFlow::matches_prefix(
            Some(flow),
            tp,
            &is_thunk_subject,
            &[&is_effect_api("failCauseSync")],
        ) {
            continue;
        }
        let lazy = ets_typeparser::parse_lazy_expression(
            flow.subject().node(),
            LazyExpressionFlags::Thunk,
        );
        // A return type annotation pins the cause type independently of the value.
        let Some(lazy) = lazy else {
            continue;
        };
        if lazy.node().type_node().is_some() {
            continue;
        }
        let inner = tp.longest_piping_flow_at(lazy.expression(), true);
        let Some(inner) = inner else {
            continue;
        };
        if inner.transformations().len() == 0 {
            continue;
        }
        let last_index = inner.transformations().len() - 1;
        let last = inner.transformations().at(last_index);
        let constructor_name = if is_cause_constructor("fail")(tp, last) {
            "fail"
        } else if allow_lazy_die && is_cause_constructor("die")(tp, last) {
            "die"
        } else {
            continue;
        };

        let fail_step = flow.transformations().at(0);
        let mut m = FailCauseToConstructorMatch {
            source_file: sf,
            location: ets_shim::scanner::get_error_range_for_node(sf, fail_step.callee()),
            replacement_name: constructor_name.to_string() + "Sync",
            source_name: "failCauseSync",
            constructor_name,
            value_node: None,
            value_type_arguments: None,
            flow: None,
            transformation_count: 0,
            lazy_callee: None,
            lazy_cause_node: None,
            lazy_expression_body: false,
        };
        let body = ast::skip_parentheses(lazy.expression());
        let value = PipingFlow::transformation_input_node(Some(&inner), last_index);
        if fail_step.kind() == TransformationKind::Call
            && no_type_arguments(last)
            && let Some(value) = value
            && body.kind() == Kind::CallExpression
            && body.as_call_expression().expression() == last.callee()
            && (constructor_name == "die" || lazy_fail_keeps_error_type(tp, fail_step, value))
        {
            m.value_node = Some(value);
            m.lazy_callee = Some(fail_step.callee());
            m.lazy_cause_node = Some(body);
            m.lazy_expression_body = !ast::is_block(lazy.body()) && lazy.expression() == body;
        }
        matches.push(m);
    }
    matches
}

// go: internal/rules/fail_cause_to_constructor.go:178 lazyFailKeepsErrorType 07e14f43ac97c387
// lazyFailKeepsErrorType reports whether Effect.failSync(() => value) infers the
// same error type as Effect.failCauseSync(() => Cause.fail(value)). Cause.fail
// infers its error from a call argument, which widens literal unions, while a
// failSync thunk returns the value against LazyArg<E>, which keeps them: for
// `cond ? "A" : "B"` the first infers string and the second "A" | "B". The fix
// is only offered when the value's own type already is the inferred error type.
//
// Rust: Go's checker argument is `tp.checker()`; Go's `effect.E() == nil` check drops (non-optional here).
pub(crate) fn lazy_fail_keeps_error_type(
    tp: &mut TypeParser<'_>,
    fail_step: &PipingFlowTransformation,
    value: P<Node>,
) -> bool {
    let out_type = fail_step.out_type(tp);
    let effect = tp.effect_type(out_type);
    let value_type = tp.get_type_at_location(value);
    let (Some(effect), Some(value_type)) = (effect, value_type) else {
        return false;
    };
    tp.checker().is_type_assignable_to(value_type, effect.e())
        && tp.checker().is_type_assignable_to(effect.e(), value_type)
}
