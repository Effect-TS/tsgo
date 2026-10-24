// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile, Symbol};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{DispatchConditionKind, PipingFlowTransformation, TypeParser};

use crate::option_match_to_from_option::{effect_module_expression, is_synchronous_function};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::Effect_0_expresses_this_conditional_validation_more_directly_than_Effect_flatMap_with_an_identity_Effect_succeed_branch_effect_flatMapConditionalToFilterOrFail;

// go: internal/rules/flat_map_conditional_to_filter_or_fail.go:17 FlatMapConditionalToFilterOrFail
// FlatMapConditionalToFilterOrFail suggests Effect.filterOrFail or
// Effect.filterOrElse when a flatMap callback implements an identity filter.
pub static FLAT_MAP_CONDITIONAL_TO_FILTER_OR_FAIL: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "flatMapConditionalToFilterOrFail",
    group: "style",
    description: "Suggests Effect.filterOrFail or Effect.filterOrElse when Effect.flatMap conditionally passes its input through with Effect.succeed",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_flat_map_conditional_to_filter_or_fail(ctx.type_parser, source_file);
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.preferred_method_name],
            ));
        }
        diagnostics
    },
});

// go: internal/rules/flat_map_conditional_to_filter_or_fail.go:44 FlatMapConditionalToFilterOrFailMatch
// FlatMapConditionalToFilterOrFailMatch holds the source evidence needed by
// the diagnostic and its quick fix.
#[derive(Clone)]
pub struct FlatMapConditionalToFilterOrFailMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub transformation: PipingFlowTransformation,
    pub effect_module_node: Option<P<Node>>,
    pub parameter_node: P<Node>,
    pub predicate_node: P<Node>,
    pub fallback_node: P<Node>,
    pub preferred_method_name: &'static str,
    pub negate_predicate: bool,
    pub can_fix: bool,
}

// go: internal/rules/flat_map_conditional_to_filter_or_fail.go:59 AnalyzeFlatMapConditionalToFilterOrFail
// AnalyzeFlatMapConditionalToFilterOrFail finds flatMap transformations whose
// callback is a two-way conditional with one identity Effect.succeed branch.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_flat_map_conditional_to_filter_or_fail(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<FlatMapConditionalToFilterOrFailMatch> {
    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index);
            let callee = transformation.callee();
            let args = transformation.args();
            if args.len() != 1 || !tp.is_node_reference_to_effect_module_api(callee, "flatMap") {
                continue;
            }
            let Some(parsed) = tp.parse_returning_dispatch(*args.at(0)) else {
                continue;
            };
            let dispatch = parsed.dispatch();
            if parsed.params().len() != 1
                || dispatch.branches().len() != 1
                || dispatch.fallback().is_none()
                || dispatch.branches().at(0).condition().kind() != DispatchConditionKind::Predicate
                || !is_synchronous_function(Some(parsed.node()))
            {
                continue;
            }

            let parameter = *parsed.params().at(0);
            let Some(parameter_name) = parameter.name() else {
                continue;
            };
            if parameter_name.kind() != Kind::Identifier {
                continue;
            }
            let declaration = parameter.as_parameter_declaration();
            if declaration.dot_dot_dot_token().is_some() || parameter.initializer().is_some() {
                continue;
            }
            let Some(parameter_symbol) = tp.get_symbol_at_location(parameter_name) else {
                continue;
            };
            if tp.checker().is_symbol_assigned(parameter_symbol) {
                continue;
            }

            let branch = *dispatch.branches().at(0);
            let fallback_result = dispatch.fallback().expect("checked above");
            let branch_identity = is_identity_succeed(tp, Some(branch.result()), parameter_symbol);
            let fallback_identity =
                is_identity_succeed(tp, Some(fallback_result), parameter_symbol);
            if branch_identity == fallback_identity {
                continue;
            }

            let mut fallback = branch.result();
            let mut negate_predicate = true;
            if branch_identity {
                fallback = fallback_result;
                negate_predicate = false;
            }

            let mut preferred_method = "filterOrElse";
            let mut fallback_node = fallback;
            if let Some(failure) = effect_fail_argument(tp, Some(fallback)) {
                preferred_method = "filterOrFail";
                fallback_node = failure;
            } else if !is_effect_expression(tp, Some(fallback)) {
                continue;
            }

            let effect_module = effect_module_expression(Some(callee));
            let m = FlatMapConditionalToFilterOrFailMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                transformation: transformation.clone(),
                effect_module_node: effect_module,
                parameter_node: parameter,
                predicate_node: branch.condition().subject(),
                fallback_node,
                preferred_method_name: preferred_method,
                negate_predicate,
                can_fix: true,
            };

            matches.push(m);
        }
    }
    matches
}

// go: internal/rules/flat_map_conditional_to_filter_or_fail.go:137 isIdentitySucceed
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn is_identity_succeed(
    tp: &mut TypeParser<'_>,
    expression: Option<P<Node>>,
    parameter_symbol: P<Symbol>,
) -> bool {
    let Some(expression) = expression.map(ast::skip_parentheses) else {
        return false;
    };
    if expression.kind() != Kind::CallExpression {
        return false;
    }
    let call = expression.as_call_expression();
    let arguments = call.arguments().nodes();
    if arguments.len() != 1
        || call.type_arguments().is_some_and(|l| !l.nodes().is_empty())
        || !tp.is_node_reference_to_effect_module_api(call.expression(), "succeed")
    {
        return false;
    }
    let argument = ast::skip_parentheses(arguments[0]);
    if argument.kind() != Kind::Identifier {
        return false;
    }
    let actual_symbol = tp.get_symbol_at_location(argument);
    actual_symbol.is_some_and(|actual_symbol| {
        tp.checker().get_symbol_if_same_reference(actual_symbol, parameter_symbol).is_some()
    })
}

// go: internal/rules/flat_map_conditional_to_filter_or_fail.go:156 effectFailArgument
pub(crate) fn effect_fail_argument(
    tp: &mut TypeParser<'_>,
    expression: Option<P<Node>>,
) -> Option<P<Node>> {
    let expression = ast::skip_parentheses(expression?);
    if expression.kind() != Kind::CallExpression {
        return None;
    }
    let call = expression.as_call_expression();
    let arguments = call.arguments().nodes();
    if arguments.len() != 1
        || call.type_arguments().is_some_and(|l| !l.nodes().is_empty())
        || !tp.is_node_reference_to_effect_module_api(call.expression(), "fail")
    {
        return None;
    }
    Some(arguments[0])
}

/// Rust-only, temporary: a copy of `internal/rules/catch_tag_to_catch_reason.go` `isEffectExpression`, whose file
/// another rule batch ports. Delete it (and use the crate-level port) when that file lands.
fn is_effect_expression(tp: &mut TypeParser<'_>, expression: Option<P<Node>>) -> bool {
    let Some(expression) = expression else {
        return false;
    };
    if tp.node_cannot_be_effect_library_type(expression) {
        return false;
    }
    let t = tp.get_type_at_location(expression);
    tp.effect_type(t).is_some()
}
