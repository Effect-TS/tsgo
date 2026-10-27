// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{LazyExpressionFlags, TypeParser, parse_lazy_expression};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_MAP_EXPRESSES_THIS_SUCCESS_VALUE_TRANSFORMATION_MORE_DIRECTLY_THAN_EFFECT_FLAT_MAP_FOLLOWED_BY_EFFECT_SUCCEED_EFFECT_FLAT_MAP_TO_MAP;

// go: internal/rules/flat_map_to_map.go:17 FlatMapToMap
// FlatMapToMap suggests using Effect.map when an Effect.flatMap callback only
// wraps its result with Effect.succeed.
pub static FLAT_MAP_TO_MAP: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "flatMapToMap",
    group: "style",
    description: "Suggests using Effect.map instead of Effect.flatMap when the callback only wraps its result with Effect.succeed",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_flat_map_to_map(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/flat_map_to_map.go:42 FlatMapToMapMatch
// FlatMapToMapMatch holds the AST nodes needed by the diagnostic and quick fix.
#[derive(Clone)]
pub struct FlatMapToMapMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub callee: P<Node>,
    pub succeed_call_expression: P<Node>,
    pub succeed_argument: P<Node>,
}

// go: internal/rules/flat_map_to_map.go:52 AnalyzeFlatMapToMap
// AnalyzeFlatMapToMap finds piping transformations whose Effect.flatMap
// callback consists solely of an Effect.succeed call.
//
// Rust: Go's checker argument is unused and dropped.
pub fn analyze_flat_map_to_map(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<FlatMapToMapMatch> {
    let mut matches = Vec::new();

    let flows = tp.piping_flows(sf, true);
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        for (_, transformation) in flow.transformations().all() {
            let callee = transformation.callee();
            let args = transformation.args();

            if args.len() == 0 {
                continue;
            }

            let callback = parse_lazy_expression(*args.at(0), LazyExpressionFlags::None);
            let Some(callback) = callback else {
                continue;
            };
            if callback.expression().kind() != Kind::CallExpression {
                continue;
            }
            let succeed_call = callback.expression().as_call_expression();
            let succeed_arguments = succeed_call.arguments().nodes();
            if succeed_arguments.len() != 1 {
                continue;
            }
            if !tp.is_node_reference_to_effect_module_api(callee, "flatMap") {
                continue;
            }
            if !tp.is_node_reference_to_effect_module_api(succeed_call.expression(), "succeed") {
                continue;
            }

            matches.push(FlatMapToMapMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                callee,
                succeed_call_expression: callback.expression(),
                succeed_argument: succeed_arguments[0],
            });
        }
    }

    matches
}
