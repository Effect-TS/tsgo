use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile, Symbol};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectMajorVersion, ResultDispatch, TypeParser};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::Effect_2_expresses_selective_recovery_more_directly_than_Effect_0_with_a_conditional_Effect_1_passthrough_effect_catchConditionalRefailToCatchIf;

// go: internal/rules/catch_conditional_refail_to_catch_if.go:17 CatchConditionalRefailToCatchIf
// CatchConditionalRefailToCatchIf detects conditional catch handlers that
// recover in one branch and re-fail the untouched handler parameter in the
// other branch.
pub static CATCH_CONDITIONAL_REFAIL_TO_CATCH_IF: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "catchConditionalRefailToCatchIf",
    group: "style",
    description: "Suggests Effect.catchIf, Effect.catchCauseIf, or Effect.catchTag for conditional catch handlers that re-fail their untouched input",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches =
            analyze_catch_conditional_refail_to_catch_if(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.catch_method_name, m.fail_method_name, m.preferred_method_name],
            ));
        }
        diagnostics
    },
});

// go: internal/rules/catch_conditional_refail_to_catch_if.go:44 CatchConditionalRefailToCatchIfMatch
#[derive(Clone)]
pub struct CatchConditionalRefailToCatchIfMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub catch_method_name: &'static str,
    pub fail_method_name: &'static str,
    pub preferred_method_name: &'static str,
}

// go: internal/rules/catch_conditional_refail_to_catch_if.go:52 conditionalRefailCatchMethods
#[derive(Clone, Copy)]
pub(crate) struct ConditionalRefailCatchMethods {
    catch_method_name: &'static str,
    fail_method_name: &'static str,
    preferred_method_name: &'static str,
}

// go: internal/rules/catch_conditional_refail_to_catch_if.go:61 AnalyzeCatchConditionalRefailToCatchIf
// AnalyzeCatchConditionalRefailToCatchIf finds exact Effect.catch and
// Effect.catchCause transformations whose handler is a single two-way
// conditional with one untouched-parameter re-fail branch.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), never nil.
pub fn analyze_catch_conditional_refail_to_catch_if(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<CatchConditionalRefailToCatchIfMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index);
            if transformation.args().len() != 1 {
                continue;
            }
            let Some(methods) = conditional_refail_methods(tp, transformation.callee()) else {
                continue;
            };

            let input_type = if index > 0 {
                flow.transformations().at(index - 1).out_type(tp)
            } else {
                flow.subject().out_type(tp)
            };
            if tp.strict_effect_type(input_type).is_none() {
                continue;
            }

            let Some(preferred_method_name) =
                analyze_conditional_refail_handler(tp, *transformation.args().at(0), methods)
            else {
                continue;
            };

            matches.push(CatchConditionalRefailToCatchIfMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
                catch_method_name: methods.catch_method_name,
                fail_method_name: methods.fail_method_name,
                preferred_method_name,
            });
        }
    }

    matches
}

// go: internal/rules/catch_conditional_refail_to_catch_if.go:107 conditionalRefailMethods
pub(crate) fn conditional_refail_methods(
    tp: &mut TypeParser<'_>,
    callee: P<Node>,
) -> Option<ConditionalRefailCatchMethods> {
    if tp.is_node_reference_to_effect_module_api(callee, "catch") {
        Some(ConditionalRefailCatchMethods {
            catch_method_name: "catch",
            fail_method_name: "fail",
            preferred_method_name: "catchIf",
        })
    } else if tp.is_node_reference_to_effect_module_api(callee, "catchCause") {
        Some(ConditionalRefailCatchMethods {
            catch_method_name: "catchCause",
            fail_method_name: "failCause",
            preferred_method_name: "catchCauseIf",
        })
    } else {
        None
    }
}

// go: internal/rules/catch_conditional_refail_to_catch_if.go:126 analyzeConditionalRefailHandler
pub(crate) fn analyze_conditional_refail_handler(
    tp: &mut TypeParser<'_>,
    handler_node: P<Node>,
    methods: ConditionalRefailCatchMethods,
) -> Option<&'static str> {
    let dispatch = tp.parse_returning_dispatch(handler_node)?;
    let result_dispatch: &ResultDispatch = dispatch.dispatch();
    if dispatch.params().len() != 1 || result_dispatch.branches().len() != 1 {
        return None;
    }
    let fallback = result_dispatch.fallback()?;
    let parameter = *dispatch.params().at(0);
    let parameter_name = parameter.name()?;
    if parameter_name.kind() != Kind::Identifier {
        return None;
    }
    let parameter_declaration = parameter.as_parameter_declaration();
    if parameter_declaration.dot_dot_dot_token().is_some()
        || parameter_declaration.initializer().is_some()
    {
        return None;
    }
    let parameter_symbol = tp.get_symbol_at_location(parameter_name)?;
    if tp.checker().is_symbol_assigned(parameter_symbol) {
        return None;
    }

    // Bare _tag dispatch is handled by catchAllTagDispatchToCatchTag. Keep the
    // generic conditional rule for inverted tag checks, which cannot be
    // represented by catchTag, and for non-tag predicates.
    let branch = *result_dispatch.branches().at(0);
    let tag_subject = ResultDispatch::common_tag_subject(Some(result_dispatch), tp);
    if tag_subject.is_some()
        && result_dispatch_tag_value(branch.condition()).is_some()
        && is_result_dispatch_tag_reference(tp, tag_subject, Some(parameter_symbol))
    {
        let branch_refails = is_conditional_refail_expression(
            tp,
            Some(branch.result()),
            parameter_symbol,
            methods.fail_method_name,
        );
        let fallback_refails = is_conditional_refail_expression(
            tp,
            Some(fallback),
            parameter_symbol,
            methods.fail_method_name,
        );
        if branch_refails == fallback_refails {
            return None;
        }
        let mut recovery = branch.result();
        if branch_refails {
            recovery = fallback;
        } else if methods.catch_method_name == "catch"
            && is_bare_parameter_tag_reference(tp, tag_subject, parameter_symbol)
        {
            return None;
        }
        if !is_effect_expression(tp, Some(recovery)) {
            return None;
        }
        return Some(methods.preferred_method_name);
    }

    let condition = branch.condition().subject();
    if !is_conditional_refail_predicate(tp, Some(condition), parameter_symbol) {
        return None;
    }

    let true_refails = is_conditional_refail_expression(
        tp,
        Some(branch.result()),
        parameter_symbol,
        methods.fail_method_name,
    );
    let false_refails = is_conditional_refail_expression(
        tp,
        Some(fallback),
        parameter_symbol,
        methods.fail_method_name,
    );
    if true_refails == false_refails {
        return None;
    }

    let mut recovery = branch.result();
    if true_refails {
        recovery = fallback;
    }
    if !is_effect_expression(tp, Some(recovery)) {
        return None;
    }

    Some(methods.preferred_method_name)
}

// go: internal/rules/catch_conditional_refail_to_catch_if.go:196 isConditionalRefailExpression
pub(crate) fn is_conditional_refail_expression(
    tp: &mut TypeParser<'_>,
    expression: Option<P<Node>>,
    parameter_symbol: P<Symbol>,
    fail_method_name: &str,
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
        || !tp.is_node_reference_to_effect_module_api(call.expression(), fail_method_name)
    {
        return false;
    }
    let argument = ast::skip_parentheses(arguments[0]);
    if argument.kind() != Kind::Identifier {
        return false;
    }
    let Some(actual_symbol) = tp.get_symbol_at_location(argument) else {
        return false;
    };
    tp.checker().get_symbol_if_same_reference(actual_symbol, parameter_symbol).is_some()
}

// go: internal/rules/catch_conditional_refail_to_catch_if.go:214 isConditionalRefailPredicate
pub(crate) fn is_conditional_refail_predicate(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
    parameter_symbol: P<Symbol>,
) -> bool {
    let Some(node) = unwrap_transparent_expression(node) else {
        return false;
    };

    match node.kind() {
        Kind::CallExpression
        | Kind::PropertyAccessExpression
        | Kind::ElementAccessExpression
        | Kind::TypeOfExpression => {
            conditional_refail_node_contains_parameter(tp, Some(node), Some(parameter_symbol))
        }
        Kind::PrefixUnaryExpression => {
            let prefix = node.as_prefix_unary_expression();
            prefix.operator() == Kind::ExclamationToken
                && is_conditional_refail_predicate(tp, Some(prefix.operand()), parameter_symbol)
        }
        Kind::BinaryExpression => {
            let binary = node.as_binary_expression();
            match binary.operator_token().kind() {
                Kind::AmpersandAmpersandToken | Kind::BarBarToken => {
                    is_conditional_refail_predicate(tp, Some(binary.left()), parameter_symbol)
                        || is_conditional_refail_predicate(
                            tp,
                            Some(binary.right()),
                            parameter_symbol,
                        )
                }
                Kind::InstanceOfKeyword => conditional_refail_node_contains_parameter(
                    tp,
                    Some(binary.left()),
                    Some(parameter_symbol),
                ),
                Kind::InKeyword => conditional_refail_node_contains_parameter(
                    tp,
                    Some(binary.right()),
                    Some(parameter_symbol),
                ),
                Kind::EqualsEqualsToken
                | Kind::EqualsEqualsEqualsToken
                | Kind::ExclamationEqualsToken
                | Kind::ExclamationEqualsEqualsToken
                | Kind::LessThanToken
                | Kind::LessThanEqualsToken
                | Kind::GreaterThanToken
                | Kind::GreaterThanEqualsToken => conditional_refail_node_contains_parameter(
                    tp,
                    Some(node),
                    Some(parameter_symbol),
                ),
                _ => false,
            }
        }
        _ => false,
    }
}

// go: internal/rules/catch_conditional_refail_to_catch_if.go:249 conditionalRefailNodeContainsParameter
pub(crate) fn conditional_refail_node_contains_parameter(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
    parameter_symbol: Option<P<Symbol>>,
) -> bool {
    let (Some(node), Some(parameter_symbol)) = (node, parameter_symbol) else {
        return false;
    };
    let mut found = false;

    // Rust: Go's recursive `visit` closure is a nested function.
    fn visit(
        tp: &mut TypeParser<'_>,
        current: P<Node>,
        parameter_symbol: P<Symbol>,
        found: &mut bool,
    ) -> bool {
        if *found {
            return *found;
        }
        if current.kind() == Kind::Identifier
            && let Some(actual) = tp.get_symbol_at_location(current)
            && tp.checker().get_symbol_if_same_reference(actual, parameter_symbol).is_some()
        {
            *found = true;
            return true;
        }
        current.for_each_child(&mut |child| visit(tp, child, parameter_symbol, found));
        *found
    }
    visit(tp, node, parameter_symbol, &mut found);
    found
}
