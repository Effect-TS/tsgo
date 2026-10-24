use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, NodeFlags, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    LazyExpressionFlags, PipingFlow, TransformationKind, TypeParser, parse_lazy_expression,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::Effect_tap_expresses_this_observation_more_directly_than_Effect_0_with_a_manual_re_yield_effect_flatMapToTap;

// go: internal/rules/flat_map_to_tap.go:15 FlatMapToTap
pub static FLAT_MAP_TO_TAP: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "flatMapToTap",
    group: "style",
    description: "Suggests Effect.tap when a flatMap or andThen callback runs an effect and returns its unchanged parameter",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_flat_map_to_tap(ctx.type_parser, source_file);
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            let mut method = "flatMap";
            if ctx.type_parser.is_node_reference_to_effect_module_api(m.callee, "andThen") {
                method = "andThen";
            }
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[method],
            ));
        }
        diagnostics
    },
});

// go: internal/rules/flat_map_to_tap.go:39 FlatMapToTapMatch
#[derive(Clone)]
pub struct FlatMapToTapMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub callee: P<Node>,
    pub return_flow: PipingFlow,
}

// go: internal/rules/flat_map_to_tap.go:47 AnalyzeFlatMapToTap
// AnalyzeFlatMapToTap finds callbacks that restore their unchanged input after an effect.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_flat_map_to_tap(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<FlatMapToTapMatch> {
    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        for (i, step) in flow.transformations().all() {
            let mut callee = step.callee();
            let mut args = step.args();
            if step.kind() == TransformationKind::Call && callee.kind() == Kind::CallExpression {
                let factory = callee.as_call_expression();
                let factory_arguments = factory.arguments().nodes();
                if factory_arguments.len() == 1
                    && factory.type_arguments().is_none()
                    && tp.is_node_reference_to_effect_module_api(factory.expression(), "andThen")
                {
                    callee = factory.expression();
                    args = ets_readonly::new(factory_arguments);
                }
            }
            if args.len() != 1
                || step.type_arguments().is_some()
                || callee.flags().intersects(NodeFlags::OptionalChain)
                || !tp.is_node_reference_to_effect_module_api(callee, "flatMap")
                    && !tp.is_node_reference_to_effect_module_api(callee, "andThen")
            {
                continue;
            }
            let callback =
                parse_lazy_expression(*args.at(0), LazyExpressionFlags::AllowMultiStatementBlock);
            let Some(callback) = callback else {
                continue;
            };
            if callback.params().len() != 1 || callback.node().type_node().is_some() {
                continue;
            }
            let parameter = *callback.params().at(0);
            if !plain_tap_parameter(parameter)
                || step.kind() == TransformationKind::Call && parameter.type_node().is_some()
            {
                continue;
            }
            let input_type = PipingFlow::transformation_input_type(Some(flow), tp, i);
            let input = tp.strict_effect_type(input_type);
            let out_type = step.out_type(tp);
            let output = tp.strict_effect_type(out_type);
            let (Some(input), Some(output)) = (input, output) else {
                continue;
            };
            if input.a() != output.a() {
                continue;
            }
            let symbol = tp.get_symbol_at_location(parameter.name());
            let Some(symbol) = symbol else {
                continue;
            };
            if symbol.declarations().len() != 1 || tp.checker().is_symbol_assigned(symbol) {
                continue;
            }
            let returned = tp.longest_piping_flow_at(callback.expression(), false);
            let Some(returned) = returned else {
                continue;
            };
            if returned.transformations().len() == 0 {
                continue;
            }
            let last = returned.transformations().len() - 1;
            let terminal = returned.transformations().at(last).clone();
            if terminal.callee().flags().intersects(NodeFlags::OptionalChain)
                || terminal.args().len() != 1
            {
                continue;
            }
            let terminal_input_type =
                PipingFlow::transformation_input_type(Some(&returned), tp, last);
            if tp.strict_effect_type(terminal_input_type).is_none() {
                continue;
            }
            let mut ref_ = *terminal.args().at(0);
            if tp.is_node_reference_to_effect_module_api(terminal.callee(), "as") {
            } else if tp.is_node_reference_to_effect_module_api(terminal.callee(), "map") {
                let mapper = parse_lazy_expression(ref_, LazyExpressionFlags::None);
                let Some(mapper) = mapper else {
                    continue;
                };
                if mapper.params().len() > 1
                    || mapper.params().len() == 1 && !plain_tap_parameter(*mapper.params().at(0))
                {
                    continue;
                }
                ref_ = mapper.expression();
            } else {
                continue;
            }
            ref_ = ast::skip_parentheses(ref_);
            if ref_.kind() != Kind::Identifier || tp.get_symbol_at_location(ref_) != Some(symbol) {
                continue;
            }
            matches.push(FlatMapToTapMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                callee,
                return_flow: returned,
            });
        }
    }
    matches
}

// go: internal/rules/flat_map_to_tap.go:122 plainTapParameter
// Rust: the parameter is the node (Go's `*ast.ParameterDeclaration` view).
pub(crate) fn plain_tap_parameter(parameter: P<Node>) -> bool {
    parameter.name().is_some_and(|name| name.kind() == Kind::Identifier)
        && parameter.initializer().is_none()
        && parameter.as_parameter_declaration().dot_dot_dot_token().is_none()
}
