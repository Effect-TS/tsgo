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
static MESSAGE: &Message = &tsdiag::Effect_mapError_expresses_the_same_error_type_transformation_more_directly_than_Effect_0_followed_by_Effect_fail_effect_catchAllToMapError;

// go: internal/rules/catch_all_to_map_error.go:16 CatchAllToMapError
// CatchAllToMapError suggests using Effect.mapError instead of Effect.catch + Effect.fail.
pub static CATCH_ALL_TO_MAP_ERROR: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "catchAllToMapError",
    group: "style",
    description: "Suggests using Effect.mapError instead of Effect.catch + Effect.fail",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_catch_all_to_map_error(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&m.catch_method_name],
            ));
        }
        diags
    },
});

// go: internal/rules/catch_all_to_map_error.go:35 CatchAllToMapErrorMatch
// CatchAllToMapErrorMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the catchAllToMapError pattern.
#[derive(Clone)]
pub struct CatchAllToMapErrorMatch {
    pub source_file: P<SourceFile>,        // The source file of the match
    pub location: TextRange,               // The pre-computed error range for this match
    pub callee: P<Node>,                   // The Effect.catch callee node (for diagnostic location)
    pub callee_name_node: Option<P<Node>>, // The "catch" name node within the PropertyAccessExpression (for text replacement)
    pub catch_method_name: String,         // The catch variant name (e.g. "catch" or "catchAll")
    pub fail_call_expression: P<Node>, // The Effect.fail(arg) call expression node (for replacement range)
    pub fail_argument: P<Node>,        // The first argument to Effect.fail (the replacement text)
}

// go: internal/rules/catch_all_to_map_error.go:47 AnalyzeCatchAllToMapError
// AnalyzeCatchAllToMapError finds all Effect.catch callbacks that simply wrap the
// error with Effect.fail, which can be simplified to Effect.mapError.
//
// Rust: Go's (ignored) checker argument is dropped.
pub fn analyze_catch_all_to_map_error(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<CatchAllToMapErrorMatch> {
    let mut matches = Vec::new();

    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (_, transformation) in flow.transformations().all() {
            if transformation.args().len() < 1 {
                continue;
            }
            let callback = *transformation.args().at(0);

            let Some(lazy) = parse_lazy_expression(callback, LazyExpressionFlags::None) else {
                continue;
            };

            let expr = lazy.expression();
            if expr.kind() != Kind::CallExpression {
                continue;
            }
            let call = expr.as_call_expression();
            let arguments = call.arguments().nodes();
            if arguments.is_empty() {
                continue;
            }

            if !tp.is_node_reference_to_effect_module_api(transformation.callee(), "catch")
                && !tp.is_node_reference_to_effect_module_api(transformation.callee(), "catchAll")
            {
                continue;
            }

            if !tp.is_node_reference_to_effect_module_api(call.expression(), "fail") {
                continue;
            }

            // Extract the "catch" name node from the PropertyAccessExpression callee
            let mut callee_name_node = None;
            let mut catch_method_name = "catch".to_string();
            let callee = transformation.callee();
            if callee.kind() == Kind::PropertyAccessExpression {
                let name = callee.as_property_access_expression().name();
                callee_name_node = Some(name);
                catch_method_name = name.text().to_string();
            }

            matches.push(CatchAllToMapErrorMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
                callee: transformation.callee(),
                callee_name_node,
                catch_method_name,
                fail_call_expression: expr,
                fail_argument: arguments[0],
            });
        }
    }

    matches
}
