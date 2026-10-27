use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_readonly::Slice;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{LazyExpressionFlags, PipingFlowTransformation, TypeParser};

use crate::is_void_expression;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::EFFECT_AS_EXPRESSES_REPLACING_THE_SUCCESS_VALUE_WITH_A_CONSTANT_DIRECTLY_EFFECT_MAP_CONST_TO_AS;

// go: internal/rules/map_const_to_as.go:17 MapConstToAs
// MapConstToAs suggests Effect.as when an Effect.map callback ignores its
// input and returns a constant.
pub static MAP_CONST_TO_AS: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "mapConstToAs",
    group: "style",
    description: "Suggests using Effect.as instead of Effect.map when the callback ignores its input and returns a constant",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let matches = analyze_map_const_to_as(ctx.type_parser, ctx.source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/map_const_to_as.go:42 MapConstToAsMatch
// MapConstToAsMatch holds the nodes needed by the diagnostic and quick fix.
#[derive(Clone)]
pub struct MapConstToAsMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub transformation: PipingFlowTransformation,
    pub effect_module_node: Option<P<Node>>, // The Effect module identifier (e.g., "Effect" in Effect.map)
    pub constant_value: P<Node>,             // The returned constant, with parentheses skipped
}

// go: internal/rules/map_const_to_as.go:53 AnalyzeMapConstToAs
// AnalyzeMapConstToAs finds data-last and data-first Effect.map calls whose
// callback ignores its input and returns a value that is stable when the
// Effect is constructed, so Effect.as can capture it once.
//
// Rust: Go's unused checker argument is dropped (the parser holds the checker); Go's
// `callee == nil` and `lazy.Expression() == nil` checks drop, both are non-optional here.
pub fn analyze_map_const_to_as(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<MapConstToAsMatch> {
    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index);
            let callee = transformation.callee();
            if transformation.args().len() != 1
                || transformation
                    .type_arguments()
                    .is_some_and(|type_arguments| !type_arguments.nodes().is_empty())
            {
                continue;
            }

            let lazy = ets_typeparser::parse_lazy_expression(
                *transformation.args().at(0),
                LazyExpressionFlags::None,
            );
            // A return type annotation pins Effect.map's success type independently
            // of the value; Effect.as infers it from the value alone.
            let Some(lazy) = lazy else {
                continue;
            };
            if lazy.node().type_node().is_some() || !is_ignorable_map_parameter_list(lazy.params())
            {
                continue;
            }
            let value = ast::skip_parentheses(lazy.expression());
            // Void-producing bodies are reported by effectMapVoid as Effect.asVoid.
            if is_void_expression(Some(value))
                || !tp.is_node_reference_to_effect_module_api(callee, "map")
                || !tp.is_expression_value_stable_at_location(value, callee)
            {
                continue;
            }

            let mut effect_module_node = None;
            if callee.kind() == Kind::PropertyAccessExpression {
                effect_module_node = Some(callee.as_property_access_expression().expression());
            }
            matches.push(MapConstToAsMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                transformation: transformation.clone(),
                effect_module_node,
                constant_value: value,
            });
        }
    }
    matches
}

// go: internal/rules/map_const_to_as.go:98 isIgnorableMapParameterList
// isIgnorableMapParameterList accepts no parameters, or a single plain
// identifier parameter. Stable values never resolve to that parameter, so it
// is unused whenever the returned value is stable.
pub(crate) fn is_ignorable_map_parameter_list(params: Slice<'_, P<Node>>) -> bool {
    match params.len() {
        0 => return true,
        1 => {
            let param = *params.at(0);
            let declaration = param.as_parameter_declaration();
            return !ast::is_this_parameter(param)
                && param.name().is_some_and(ast::is_identifier)
                && declaration.dot_dot_dot_token().is_none()
                && declaration.initializer().is_none();
        }
        _ => {}
    }
    false
}
