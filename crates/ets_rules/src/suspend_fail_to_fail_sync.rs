use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, LazyExpressionFlags, ParsedLazyExpression, PipingFlow, PipingFlowSubject,
    PipingFlowTransformation, TypeParser,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_0_BUILDS_THIS_1_LAZILY_DIRECTLY_INSTEAD_OF_WRAPPING_EFFECT_2_IN_EFFECT_SUSPEND_EFFECT_SUSPEND_FAIL_TO_FAIL_SYNC;

// go: internal/rules/suspend_fail_to_fail_sync.go:17 SuspendFailToFailSync cabbf9439fbbff9b
// SuspendFailToFailSync suggests Effect.failSync (or Effect.dieSync on Effect
// v3) when an Effect.suspend thunk only returns a flow ending in Effect.fail
// (or Effect.die).
pub static SUSPEND_FAIL_TO_FAIL_SYNC: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "suspendFailToFailSync",
    group: "style",
    description: "Suggests Effect.failSync/dieSync instead of Effect.suspend wrapping a single Effect.fail/die",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_suspend_fail_to_fail_sync(ctx.type_parser, Some(source_file));
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.replacement_name, m.kind, m.inner_name],
            ));
        }
        diags
    },
});

// go: internal/rules/suspend_fail_to_fail_sync.go:45 SuspendFailToFailSyncMatch d7e4903da3dea9d7
// SuspendFailToFailSyncMatch holds the nodes needed by the diagnostic and quick fix.
//
// Rust: Go's `*PipingFlow` is an owned copy.
#[derive(Clone)]
pub struct SuspendFailToFailSyncMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub replacement_name: &'static str, // failSync or dieSync
    pub kind: &'static str,             // error or defect
    pub inner_name: &'static str,       // fail or die
    // Suspend is the Effect.suspend step; the thunk is the subject of its flow.
    pub suspend: PipingFlowTransformation,
    pub thunk: ParsedLazyExpression,
    // Inner is the flow of the thunk's returned expression, which it covers
    // whole; its last step is Effect.fail or Effect.die.
    pub inner: PipingFlow,
}

// go: internal/rules/suspend_fail_to_fail_sync.go:62 AnalyzeSuspendFailToFailSync b4bbc0e9ec8b5072
// AnalyzeSuspendFailToFailSync finds piping flows that apply Effect.suspend to
// a thunk whose returned flow ends in Effect.fail, or Effect.die on Effect v3,
// which has Effect.dieSync.
//
// Rust: Go's checker argument is the parser's checker, so it is dropped; Go's nil parser check has no counterpart
// (the parser is a reference). Go's `Callee() != nil`, `thunk.Expression() == nil` and `inner == nil` checks drop:
// all three are non-optional here.
pub fn analyze_suspend_fail_to_fail_sync(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<SuspendFailToFailSyncMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    let allow_die = tp.supported_effect_version() != EffectMajorVersion::V4;

    let no_type_arguments = |transformation: &PipingFlowTransformation| -> bool {
        transformation.type_arguments().is_none_or(|arguments| arguments.nodes().is_empty())
    };
    let is_effect_api = |name: &'static str| {
        move |tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| -> bool {
            transformation.args().len() == 0
                && tp.is_node_reference_to_effect_module_api(transformation.callee(), name)
        }
    };
    // An arrow or function expression is always the subject of its flow, so
    // Effect.suspend is the first transformation.
    let is_thunk_subject = |_: &mut TypeParser<'_>, subject: &PipingFlowSubject| -> bool {
        subject.node().is_some_and(|node| {
            node.kind() == Kind::ArrowFunction || node.kind() == Kind::FunctionExpression
        })
    };
    // Explicit type arguments on suspend pin an error channel that failSync would narrow.
    let is_suspend = |tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| -> bool {
        no_type_arguments(transformation) && is_effect_api("suspend")(tp, transformation)
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        if !PipingFlow::matches_prefix(Some(flow), tp, &is_thunk_subject, &[&is_suspend]) {
            continue;
        }
        let thunk = ets_typeparser::parse_lazy_expression(
            flow.subject().node(),
            LazyExpressionFlags::Thunk,
        );
        // A return type annotation pins the error channel independently of the value.
        let Some(thunk) = thunk else {
            continue;
        };
        if thunk.node().type_node().is_some() {
            continue;
        }
        let Some(inner) = tp.longest_piping_flow_at(thunk.expression(), true) else {
            continue;
        };
        if inner.node() != ast::skip_parentheses(thunk.expression())
            || inner.transformations().len() == 0
        {
            continue;
        }
        let last = inner.transformations().at(inner.transformations().len() - 1).clone();

        let suspend = flow.transformations().at(0).clone();
        let mut m = SuspendFailToFailSyncMatch {
            source_file: sf,
            location: ets_shim::scanner::get_error_range_for_node(sf, suspend.callee()),
            replacement_name: "",
            kind: "",
            inner_name: "",
            suspend,
            thunk,
            inner,
        };
        if is_effect_api("fail")(tp, &last) {
            (m.replacement_name, m.kind, m.inner_name) = ("failSync", "error", "fail");
        } else if allow_die && is_effect_api("die")(tp, &last) {
            (m.replacement_name, m.kind, m.inner_name) = ("dieSync", "defect", "die");
        } else {
            continue;
        }
        matches.push(m);
    }
    matches
}
