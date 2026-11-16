use std::cell::Cell;
use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile, Symbol};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    DispatchCondition, DispatchConditionKind, PipingFlow, PipingFlowSubject,
    PipingFlowTransformation, TypeParser,
};

use crate::*;

/// Rust-only: Go's message identifier, bound once so rustfmt can lay out the `pub static`.
static MESSAGE: &Message = &tsdiag::EFFECT_ON_ERROR_EXPRESSES_THIS_FAILURE_ONLY_EFFECT_ON_EXIT_CALLBACK_MORE_DIRECTLY_EFFECT_ON_EXIT_FAILURE_TO_ON_ERROR;

// go: internal/rules/on_exit_failure_to_on_error.go:14 OnExitFailureToOnError b80f63c265ec646d
pub static ON_EXIT_FAILURE_TO_ON_ERROR: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "onExitFailureToOnError",
    group: "style",
    description: "Suggests Effect.onError when an Effect.onExit callback only acts on failure",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let matches = analyze_on_exit_failure_to_on_error(ctx.type_parser, Some(ctx.source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/on_exit_failure_to_on_error.go:38 OnExitFailureToOnErrorMatch a4234c2fa5c55de4
#[derive(Clone)]
pub struct OnExitFailureToOnErrorMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
}

// go: internal/rules/on_exit_failure_to_on_error.go:43 AnalyzeOnExitFailureToOnError 1451915cf7268e88
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), so it is dropped. A transformation's
// callee is never nil, so Go's nil check drops.
pub fn analyze_on_exit_failure_to_on_error(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<OnExitFailureToOnErrorMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        let sequences = PipingFlow::find_transformation_sequences(
            Some(flow),
            tp,
            &[&|tp: &mut TypeParser<'_>, t: &PipingFlowTransformation| {
                if t.args().len() != 1 {
                    return false;
                }
                let callback = ast::skip_parentheses(*t.args().at(0));
                if callback.kind() != Kind::ArrowFunction
                    && callback.kind() != Kind::FunctionExpression
                {
                    return false;
                }
                tp.is_node_reference_to_effect_module_api(t.callee(), "onExit")
            }],
        );
        for sequence in sequences.as_deref().unwrap_or_default() {
            let transformation = sequence.transformations().at(0);
            if let Some(m) = analyze_on_exit_failure_callback(tp, sf, transformation) {
                matches.push(m);
            }
        }
    }
    matches
}

// go: internal/rules/on_exit_failure_to_on_error.go:70 analyzeOnExitFailureCallback 5a7e040f7d1ca44e
//
// Rust: Go's `(match, ok)` is an `Option`; the checker is the parser's. The transformation is borrowed, not
// copied as in Go, since the match does not keep it.
pub(crate) fn analyze_on_exit_failure_callback(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    transformation: &PipingFlowTransformation,
) -> Option<OnExitFailureToOnErrorMatch> {
    let callback = ast::skip_parentheses(*transformation.args().at(0));
    if !is_synchronous_function(Some(callback)) {
        return None;
    }
    let parsed = tp.parse_returning_dispatch_allowing_type_parameters(callback)?;
    if parsed.params().len() != 1 {
        return None;
    }
    let dispatch = parsed.dispatch();
    if dispatch.branches().len() != 1 || dispatch.fallback().is_none() {
        return None;
    }

    let parameter = *parsed.params().at(0);
    let parameter_name = parameter.name()?;
    if parameter_name.kind() != Kind::Identifier {
        return None;
    }
    let declaration = parameter.as_parameter_declaration();
    if declaration.dot_dot_dot_token().is_some() || declaration.initializer().is_some() {
        return None;
    }
    let parameter_symbol = tp.get_symbol_at_location(parameter_name)?;
    if tp.checker().is_symbol_assigned(parameter_symbol) {
        return None;
    }

    let branch = *dispatch.branches().at(0);
    let failure_when_true =
        on_exit_condition_checks_failure(tp, branch.condition(), parameter_symbol)?;

    let mut noop = dispatch.fallback().unwrap();
    if !failure_when_true {
        noop = branch.result();
    }
    if !is_effect_void_reference(tp, Some(noop)) {
        return None;
    }

    Some(OnExitFailureToOnErrorMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
    })
}

// go: internal/rules/on_exit_failure_to_on_error.go:117 onExitConditionChecksFailure e2db8f838d6638b5
//
// Rust: Go's `(failureWhenTrue, ok)` is `Some(failureWhenTrue)` when ok; the checker is the parser's.
pub(crate) fn on_exit_condition_checks_failure(
    tp: &mut TypeParser<'_>,
    condition: DispatchCondition,
    parameter_symbol: P<Symbol>,
) -> Option<bool> {
    match condition.kind() {
        DispatchConditionKind::SwitchCase => {
            if !is_bare_parameter_tag_reference(tp, condition.tag_subject(), parameter_symbol) {
                return None;
            }
            let value = result_dispatch_tag_value(condition)?;
            exit_tag_is_failure(&value)
        }
        DispatchConditionKind::Predicate => {
            decode_on_exit_failure_predicate(tp, Some(condition.subject()), parameter_symbol, false)
        }
    }
}

// go: internal/rules/on_exit_failure_to_on_error.go:135 decodeOnExitFailurePredicate 42a624c546b75642
//
// Rust: Go's `(isFailure, ok)` is `Some(isFailure)` when ok; the checker is the parser's. A binary expression's
// operator token and operands are never nil, so Go's nil checks drop.
pub(crate) fn decode_on_exit_failure_predicate(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
    parameter_symbol: P<Symbol>,
    negated: bool,
) -> Option<bool> {
    let predicate = unwrap_transparent_expression(node)?;

    match predicate.kind() {
        Kind::PrefixUnaryExpression => {
            let unary = predicate.as_prefix_unary_expression();
            if unary.operator() != Kind::ExclamationToken {
                return None;
            }
            decode_on_exit_failure_predicate(tp, Some(unary.operand()), parameter_symbol, !negated)
        }

        Kind::BinaryExpression => {
            let binary = predicate.as_binary_expression();
            if binary.operator_token().kind() != Kind::EqualsEqualsEqualsToken {
                return None;
            }
            match unwrap_transparent_expression(Some(binary.right())).map(|right| right.kind()) {
                Some(Kind::TrueKeyword) => {
                    return decode_on_exit_failure_predicate(
                        tp,
                        Some(binary.left()),
                        parameter_symbol,
                        negated,
                    );
                }
                Some(Kind::FalseKeyword) => {
                    return decode_on_exit_failure_predicate(
                        tp,
                        Some(binary.left()),
                        parameter_symbol,
                        !negated,
                    );
                }
                _ => {}
            }
            let (subject, value) = tp.parse_tag_match(predicate);
            let value = unwrap_transparent_expression(value);
            let (Some(subject), Some(value)) = (subject, value) else {
                return None;
            };
            if !ast::is_string_literal(value)
                || !is_bare_parameter_tag_reference(tp, Some(subject), parameter_symbol)
            {
                return None;
            }
            let is_failure = exit_tag_is_failure(value.as_string_literal().text())?;
            Some(is_failure != negated)
        }

        Kind::CallExpression => {
            let flow = tp.longest_piping_flow_at(predicate, false)?;
            let is_failure = Cell::new(false);
            let matched = PipingFlow::matches_exactly(
                Some(&flow),
                tp,
                &|tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| -> bool {
                    let node = unwrap_transparent_expression(subject.node());
                    node.is_some_and(|node| {
                        node.kind() == Kind::Identifier
                            && same_catch_reason_symbol(tp, Some(node), Some(parameter_symbol))
                    })
                },
                &[&|tp: &mut TypeParser<'_>, t: &PipingFlowTransformation| -> bool {
                    if t.args().len() != 0 {
                        return false;
                    }
                    if tp.is_node_reference_to_effect_exit_module_api(t.callee(), "isFailure") {
                        is_failure.set(true);
                        return true;
                    }
                    tp.is_node_reference_to_effect_exit_module_api(t.callee(), "isSuccess")
                }],
            );
            if !matched {
                return None;
            }
            Some(is_failure.get() != negated)
        }
        _ => None,
    }
}

// go: internal/rules/on_exit_failure_to_on_error.go:203 exitTagIsFailure 640fe63e4f16a9a5
//
// Rust: Go's `(isFailure, ok)` is `Some(isFailure)` when ok.
pub(crate) fn exit_tag_is_failure(tag: &str) -> Option<bool> {
    match tag {
        "Failure" => Some(true),
        "Success" => Some(false),
        _ => None,
    }
}
