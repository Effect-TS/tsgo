use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, LazyExpressionFlags, ParsedLazyExpression, PipingFlow, PipingFlowSubject,
    PipingFlowTransformation, TypeParser,
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
    replaced_by: vec![],
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

// go: internal/rules/fail_cause_to_constructor.go:45 FailCauseToConstructorMatch 66dee876026e00f2
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
    // Flow's first TransformationCount steps end in Effect.failCause, after
    // Cause.fail or Cause.die, or are the single Effect.failCauseSync step
    // applied to Lazy.
    pub flow: Option<PipingFlow>,
    pub transformation_count: i64,
    // Lazy is the failCauseSync thunk, and Inner the flow of its returned
    // expression, which ends in Cause.fail or Cause.die; both are nil for
    // Effect.failCause.
    pub lazy: Option<ParsedLazyExpression>,
    pub inner: Option<PipingFlow>,
}

// go: internal/rules/fail_cause_to_constructor.go:67 AnalyzeFailCauseToConstructor 936414918167cead
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
                let fail_step = flow.transformations().at(sequence.start() + 1);
                matches.push(FailCauseToConstructorMatch {
                    source_file: sf,
                    location: ets_shim::scanner::get_error_range_for_node(sf, fail_step.callee()),
                    replacement_name: constructor_name.to_string(),
                    source_name: "failCause",
                    constructor_name,
                    flow: Some(flow.clone()),
                    transformation_count: sequence.start() + 2,
                    lazy: None,
                    inner: None,
                });
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
        let last = inner.transformations().at(inner.transformations().len() - 1);
        let constructor_name = if is_cause_constructor("fail")(tp, last) {
            "fail"
        } else if allow_lazy_die && is_cause_constructor("die")(tp, last) {
            "die"
        } else {
            continue;
        };

        matches.push(FailCauseToConstructorMatch {
            source_file: sf,
            location: ets_shim::scanner::get_error_range_for_node(
                sf,
                flow.transformations().at(0).callee(),
            ),
            replacement_name: constructor_name.to_string() + "Sync",
            source_name: "failCauseSync",
            constructor_name,
            flow: Some(flow.clone()),
            transformation_count: 1,
            lazy: Some(lazy),
            inner: Some(inner),
        });
    }
    matches
}
