use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{LazyExpressionFlags, TypeParser, parse_lazy_expression};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_AND_THEN_EXPRESSES_THIS_SEQUENCING_MORE_DIRECTLY_THAN_EFFECT_FLAT_MAP_WITH_A_ZERO_PARAMETER_CALLBACK_EFFECT_FLAT_MAP_IGNORED_PARAM_TO_AND_THEN;

// go: internal/rules/flat_map_ignored_param_to_and_then.go:16 FlatMapIgnoredParamToAndThen f4c577d5ef3501c7
// FlatMapIgnoredParamToAndThen suggests Effect.andThen when an Effect.flatMap
// callback ignores the upstream value and returns an existing Effect value.
pub static FLAT_MAP_IGNORED_PARAM_TO_AND_THEN: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "flatMapIgnoredParamToAndThen",
    group: "style",
    description: "Suggests using Effect.andThen instead of Effect.flatMap when a zero-parameter callback returns an existing Effect value",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_flat_map_ignored_param_to_and_then(ctx.type_parser, source_file);
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/flat_map_ignored_param_to_and_then.go:42 FlatMapIgnoredParamToAndThenMatch 3cc8cff9a6d38ad3
// FlatMapIgnoredParamToAndThenMatch holds the nodes needed by the diagnostic
// and quick fix.
#[derive(Clone)]
pub struct FlatMapIgnoredParamToAndThenMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub callee: P<Node>,
    pub callback: P<Node>,
    pub effect_value: P<Node>,
}

// go: internal/rules/flat_map_ignored_param_to_and_then.go:53 AnalyzeFlatMapIgnoredParamToAndThen 4325680290c2aaf3
// AnalyzeFlatMapIgnoredParamToAndThen finds Effect.flatMap piping
// transformations whose zero-parameter expression callback returns an Effect
// value held by an already-initialized const binding.
//
// Rust: Go's checker argument is unused and dropped.
pub fn analyze_flat_map_ignored_param_to_and_then(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<FlatMapIgnoredParamToAndThenMatch> {
    let mut matches = Vec::new();

    let flows = tp.piping_flows(sf, true);
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        for (_, transformation) in flow.transformations().all() {
            let callee = transformation.callee();
            if transformation.args().len() != 1
                || !tp.is_node_reference_to_effect_module_api(callee, "flatMap")
            {
                continue;
            }

            let callback =
                parse_lazy_expression(*transformation.args().at(0), LazyExpressionFlags::Thunk);
            let Some(callback) = callback else {
                continue;
            };
            let expression = callback.expression();
            if tp.node_cannot_be_effect_library_type(expression)
                || !tp.is_expression_value_stable_at_location(expression, callee)
            {
                continue;
            }
            let t = tp.get_type_at_location(expression);
            if tp.effect_type(t).is_none() {
                continue;
            }

            matches.push(FlatMapIgnoredParamToAndThenMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                callee,
                callback: callback.node(),
                effect_value: expression,
            });
        }
    }

    matches
}
