use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Node, SourceFile};
use ets_shim::checker::TypeFlags;
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectMajorVersion, PipingFlow, PipingFlowTransformation, TypeParser};

use crate::piping_flow::has_explicit_type_arguments;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_OR_DIE_EXPRESSES_ESCALATING_EVERY_TYPED_FAILURE_INTO_A_DEFECT_MORE_DIRECTLY_THAN_EFFECT_0_WITH_AN_IDENTITY_FORWARDING_EFFECT_DIE_HANDLER_EFFECT_CATCH_DIE_TO_OR_DIE;

// go: internal/rules/catch_die_to_or_die.go:16 CatchDieToOrDie
// CatchDieToOrDie suggests using Effect.orDie instead of a catch-all
// handler that forwards the typed failure unchanged to Effect.die.
pub static CATCH_DIE_TO_OR_DIE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "catchDieToOrDie",
    group: "style",
    description: "Suggests using Effect.orDie instead of Effect.catch or Effect.catchAll with an identity-forwarding Effect.die handler",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_catch_die_to_or_die(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.catch_method_name],
            ));
        }
        diagnostics
    },
});

// go: internal/rules/catch_die_to_or_die.go:42 CatchDieToOrDieMatch
// CatchDieToOrDieMatch holds the nodes needed by the diagnostic and fix.
#[derive(Clone)]
pub struct CatchDieToOrDieMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub transformation: PipingFlowTransformation,
    pub catch_method_name: &'static str,
    pub has_type_arguments: bool,
}

// go: internal/rules/catch_die_to_or_die.go:52 AnalyzeCatchDieToOrDie
// AnalyzeCatchDieToOrDie finds Effect.catch/catchAll transformations whose
// handler is Effect.die or forwards its sole argument unchanged to Effect.die.
//
// Rust: Go's (ignored) checker argument is dropped.
pub fn analyze_catch_die_to_or_die(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<CatchDieToOrDieMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (i, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(i).clone();
            let callee = transformation.callee();
            let args = transformation.args();
            let method = catch_die_method(tp, Some(callee));
            let Some(method_name) = method else {
                continue;
            };
            if args.len() != 1 {
                continue;
            }
            let (handler, _, _) = tp.unwrap_identity_forwarder(*args.at(0));
            let Some(handler) = handler else {
                continue;
            };
            if !tp.is_node_reference_to_effect_module_api(handler, "die") {
                continue;
            }
            let input_type = PipingFlow::transformation_input_type(Some(flow), tp, i);
            let input_effect = tp.strict_effect_type(input_type);
            if let Some(input_effect) = input_effect
                && !input_effect.e().flags().intersects(TypeFlags::Never)
            {
                matches.push(CatchDieToOrDieMatch {
                    source_file: sf,
                    location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                    catch_method_name: method_name,
                    has_type_arguments: has_explicit_type_arguments(Some(&transformation)),
                    transformation,
                });
            }
        }
    }

    matches
}

// go: internal/rules/catch_die_to_or_die.go:87 catchDieMethod
//
// Rust: Go's `(name, ok)` result is `Some(name)` when ok.
pub(crate) fn catch_die_method(
    tp: &mut TypeParser<'_>,
    callee: Option<P<Node>>,
) -> Option<&'static str> {
    let callee = callee?;

    let mut name = "catchAll";
    if tp.supported_effect_version() == EffectMajorVersion::V4 {
        name = "catch";
    }
    if !tp.is_node_reference_to_effect_module_api(callee, name) {
        return None;
    }

    Some(name)
}
