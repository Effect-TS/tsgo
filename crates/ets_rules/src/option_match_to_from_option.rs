use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, ModifierFlags, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{
    DispatchConditionKind, EffectMajorVersion, LazyExpressionFlags, PipingFlowTransformation,
    TransformationKind, TypeParser,
};

// go: internal/rules/option_match_to_from_option.go:17 OptionMatchToFromOption 61343ea08c92b448
// OptionMatchToFromOption suggests Effect.fromOption when Option.match or an
// Option tag conditional only lifts Some with Effect.succeed and None with
// Effect.fail.
pub static OPTION_MATCH_TO_FROM_OPTION: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "optionMatchToFromOption",
        group: "style",
        description: "Suggests Effect.fromOption when Option.match or an Option tag conditional only converts Some to Effect.succeed and None to Effect.fail",
        default_severity: Severity::Suggestion,
        supported_effect: vec!["v4"],
        codes: vec![
            tsdiag::EFFECT_FROM_OPTION_EXPRESSES_THIS_OPTION_TO_EFFECT_CONVERSION_MORE_DIRECTLY_THAN_OPTION_MATCH_OR_AN_OPTION_TAG_CONDITIONAL_EFFECT_OPTION_MATCH_TO_FROM_OPTION.code(),
        ],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            let matches = analyze_option_match_to_from_option(ctx.type_parser, Some(ctx.source_file));
            let mut diagnostics = Vec::with_capacity(matches.len());
            for m in &matches {
                diagnostics.push(ctx.new_diagnostic(
                    m.source_file,
                    m.location,
                    &tsdiag::EFFECT_FROM_OPTION_EXPRESSES_THIS_OPTION_TO_EFFECT_CONVERSION_MORE_DIRECTLY_THAN_OPTION_MATCH_OR_AN_OPTION_TAG_CONDITIONAL_EFFECT_OPTION_MATCH_TO_FROM_OPTION,
                    &[],
                    &[],
                ));
            }
            diagnostics
        },
    }
});

// go: internal/rules/option_match_to_from_option.go:43 OptionMatchToFromOptionMatch a695c2d64d61a3fc
// OptionMatchToFromOptionMatch holds the nodes needed by the diagnostic and
// quick fix.
//
// Rust: Go's `*typeparser.PipingFlowTransformation` points at a copy of the flow's transformation; the match owns
// that copy.
#[derive(Clone)]
pub struct OptionMatchToFromOptionMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub transformation: Option<PipingFlowTransformation>,
    pub replacement_node: Option<P<Node>>,
    pub option_node: Option<P<Node>>,
    pub failure_node: Option<P<Node>>,
    pub default_failure: bool,
    pub can_fix: bool,
}

// go: internal/rules/option_match_to_from_option.go:56 AnalyzeOptionMatchToFromOption 3549b0241607076e
// AnalyzeOptionMatchToFromOption finds Option.match and Option result
// dispatches that are equivalent to Effect.fromOption.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), so it is dropped.
pub fn analyze_option_match_to_from_option(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<OptionMatchToFromOptionMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = analyze_option_match_calls(tp, sf);
    matches.extend(analyze_option_conditionals(tp, sf));
    matches
}

// go: internal/rules/option_match_to_from_option.go:66 analyzeOptionMatchCalls a35d6e3269b078d0
pub(crate) fn analyze_option_match_calls(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<OptionMatchToFromOptionMatch> {
    let mut matches = Vec::new();

    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index);
            let callee = transformation.callee();
            let args = transformation.args();
            match transformation.kind() {
                TransformationKind::Pipe
                | TransformationKind::Pipeable
                | TransformationKind::DataFirst
                | TransformationKind::DataLast
                | TransformationKind::Call => {}
                _ => continue,
            }

            if args.len() != 1 || !tp.is_node_reference_to_effect_option_module_api(callee, "match")
            {
                continue;
            }

            let Some((failure, can_fix)) = analyze_option_match_handlers(tp, Some(*args.at(0)))
            else {
                continue;
            };

            let m = OptionMatchToFromOptionMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                transformation: Some(transformation.clone()),
                replacement_node: None,
                option_node: None,
                failure_node: failure,
                default_failure: is_default_no_such_element_error(tp, failure),
                can_fix,
            };

            matches.push(m);
        }
    }

    matches
}

// go: internal/rules/option_match_to_from_option.go:109 analyzeOptionMatchHandlers b5735757c78d2f4b
//
// Rust: Go's `(failure, canFix, ok)` is an `Option` of the first two.
pub(crate) fn analyze_option_match_handlers(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> Option<(Option<P<Node>>, bool)> {
    let node = ast::skip_parentheses(node?);
    if node.kind() != Kind::ObjectLiteralExpression {
        return None;
    }
    let object = node.as_object_literal_expression();
    if object.properties.nodes().len() != 2 {
        return None;
    }

    let mut on_none: Option<P<Node>> = None;
    let mut on_some: Option<P<Node>> = None;
    for &property_node in object.properties.nodes() {
        if property_node.kind() != Kind::PropertyAssignment {
            return None;
        }
        let name = property_node.name()?;
        if name.kind() != Kind::Identifier {
            return None;
        }
        let initializer = property_node.initializer()?;
        match ets_shim::scanner::get_text_of_node(name).as_str() {
            "onNone" => {
                if on_none.is_some() {
                    return None;
                }
                on_none = Some(initializer);
            }
            "onSome" => {
                if on_some.is_some() {
                    return None;
                }
                on_some = Some(initializer);
            }
            _ => return None,
        }
    }

    if !is_effect_succeed_handler(tp, on_some) {
        return None;
    }
    analyze_effect_fail_handler(tp, on_none)
}

// go: internal/rules/option_match_to_from_option.go:150 isEffectSucceedHandler 53f69e0934085a80
pub(crate) fn is_effect_succeed_handler(tp: &mut TypeParser<'_>, node: Option<P<Node>>) -> bool {
    let (target, type_arguments, _) = tp.unwrap_identity_forwarder(node);
    type_arguments.is_none_or(|type_arguments| type_arguments.nodes().is_empty())
        && target.is_some_and(|target| tp.is_node_reference_to_effect_module_api(target, "succeed"))
}

// go: internal/rules/option_match_to_from_option.go:155 analyzeEffectFailHandler 1d9f9900cfcd9560
//
// Rust: Go's `(failure, canFix, ok)` is an `Option` of the first two.
pub(crate) fn analyze_effect_fail_handler(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> Option<(Option<P<Node>>, bool)> {
    let lazy = ets_typeparser::parse_lazy_expression(node, LazyExpressionFlags::Thunk)?;
    let expression = ast::skip_parentheses(lazy.expression());
    if expression.kind() != Kind::CallExpression {
        return None;
    }
    let call = expression.as_call_expression();
    if call.arguments.nodes().len() != 1
        || !expression.type_arguments().is_empty()
        || !tp.is_node_reference_to_effect_module_api(call.expression, "fail")
    {
        return None;
    }
    Some((Some(call.arguments.nodes()[0]), lazy.node().kind() == Kind::ArrowFunction))
}

// go: internal/rules/option_match_to_from_option.go:173 isSynchronousFunction 612b5746617f1e67
pub(crate) fn is_synchronous_function(node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if ast::get_combined_modifier_flags(node).intersects(ModifierFlags::Async) {
        return false;
    }
    if node.kind() == Kind::FunctionExpression {
        return node.as_function_expression().asterisk_token().is_none();
    }
    node.kind() == Kind::ArrowFunction
}

// go: internal/rules/option_match_to_from_option.go:184 analyzeOptionConditionals bb2bb5f98661c5ac
//
// Rust: the checker is the parser's.
pub(crate) fn analyze_option_conditionals(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<OptionMatchToFromOptionMatch> {
    let mut matches = Vec::new();
    // Rust: Go's recursive `visit` closure is a nested function.
    fn visit(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<OptionMatchToFromOptionMatch>,
    ) -> bool {
        if node.kind() == Kind::ConditionalExpression
            && let Some(m) = analyze_option_conditional(tp, sf, node)
        {
            matches.push(m);
        }
        node.for_each_child(&mut |child| visit(tp, sf, child, matches));
        false
    }
    sf.as_node().for_each_child(&mut |child| visit(tp, sf, child, &mut matches));
    matches
}

// go: internal/rules/option_match_to_from_option.go:203 analyzeOptionConditional 6996d5cbe31141c9
//
// Rust: Go's `(match, ok)` is an `Option`; the checker is the parser's.
pub(crate) fn analyze_option_conditional(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<OptionMatchToFromOptionMatch> {
    let dispatch = tp.parse_result_dispatch(node)?;
    if dispatch.branches().len() != 1 || dispatch.fallback().is_none() {
        return None;
    }
    let branch = *dispatch.branches().at(0);
    if branch.condition().kind() != DispatchConditionKind::Predicate
        || branch.condition().value().is_some()
    {
        return None;
    }

    let condition_node = ast::skip_parentheses(branch.condition().subject());
    if condition_node.kind() != Kind::CallExpression {
        return None;
    }
    let condition = condition_node.as_call_expression();
    if condition.arguments.nodes().len() != 1 || !condition_node.type_arguments().is_empty() {
        return None;
    }

    let option_node = ast::skip_parentheses(condition.arguments.nodes()[0]);
    if option_node.kind() != Kind::Identifier {
        return None;
    }

    let is_some = tp.is_node_reference_to_effect_option_module_api(condition.expression, "isSome");
    let is_none = tp.is_node_reference_to_effect_option_module_api(condition.expression, "isNone");
    if is_some == is_none {
        return None;
    }

    let mut succeed_node = Some(branch.result());
    let mut fail_node = dispatch.fallback();
    if is_none {
        std::mem::swap(&mut succeed_node, &mut fail_node);
    }

    if !is_conditional_succeed(tp, Some(option_node), succeed_node) {
        return None;
    }
    let failure = analyze_effect_fail_call(tp, fail_node)?;

    Some(OptionMatchToFromOptionMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, condition.expression),
        transformation: None,
        replacement_node: Some(dispatch.node()),
        option_node: Some(option_node),
        failure_node: failure,
        default_failure: is_default_no_such_element_error(tp, failure),
        can_fix: true,
    })
}

// go: internal/rules/option_match_to_from_option.go:259 isConditionalSucceed 28e2f4991fd55d5a
//
// Rust: the checker is the parser's.
pub(crate) fn is_conditional_succeed(
    tp: &mut TypeParser<'_>,
    option_node: Option<P<Node>>,
    node: Option<P<Node>>,
) -> bool {
    let Some(node) = node else {
        return false;
    };
    let expression = ast::skip_parentheses(node);
    if expression.kind() != Kind::CallExpression {
        return false;
    }
    let call = expression.as_call_expression();
    if call.arguments.nodes().len() != 1
        || !expression.type_arguments().is_empty()
        || !tp.is_node_reference_to_effect_module_api(call.expression, "succeed")
    {
        return false;
    }

    let value_node = ast::skip_parentheses(call.arguments.nodes()[0]);
    if value_node.kind() != Kind::PropertyAccessExpression {
        return false;
    }
    let value = value_node.as_property_access_expression();
    if value_node.question_dot_token().is_some()
        || ets_shim::scanner::get_text_of_node(value.name) != "value"
        || !same_identifier_reference(
            tp,
            option_node,
            Some(ast::skip_parentheses(value.expression)),
        )
    {
        return false;
    }
    true
}

// go: internal/rules/option_match_to_from_option.go:283 analyzeEffectFailCall 00e9735250958be4
//
// Rust: Go's `(failure, ok)` is an `Option` of the failure.
#[expect(clippy::option_option, reason = "Go's (node, ok) with a node that may be nil when ok")]
pub(crate) fn analyze_effect_fail_call(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> Option<Option<P<Node>>> {
    let expression = ast::skip_parentheses(node?);
    if expression.kind() != Kind::CallExpression {
        return None;
    }
    let call = expression.as_call_expression();
    if call.arguments.nodes().len() != 1
        || !expression.type_arguments().is_empty()
        || !tp.is_node_reference_to_effect_module_api(call.expression, "fail")
    {
        return None;
    }
    Some(Some(call.arguments.nodes()[0]))
}

// go: internal/rules/option_match_to_from_option.go:297 sameIdentifierReference eca2888e33593d7c
//
// Rust: the checker is the parser's.
pub(crate) fn same_identifier_reference(
    tp: &mut TypeParser<'_>,
    left: Option<P<Node>>,
    right: Option<P<Node>>,
) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };
    if left.kind() != Kind::Identifier || right.kind() != Kind::Identifier {
        return false;
    }
    let left_symbol = tp.get_symbol_at_location(left);
    let right_symbol = tp.get_symbol_at_location(right);
    left_symbol.is_some()
        && right_symbol.is_some()
        && ets_typeparser::get_symbol_if_same_reference(tp.checker(), left_symbol, right_symbol)
            .is_some()
}

// go: internal/rules/option_match_to_from_option.go:306 isDefaultNoSuchElementError 760fc7ce925cd416
pub(crate) fn is_default_no_such_element_error(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> bool {
    let Some(node) = node else {
        return false;
    };
    let node = ast::skip_parentheses(node);
    if node.kind() != Kind::NewExpression {
        return false;
    }
    let expression = node.as_new_expression();
    if !node.type_arguments().is_empty()
        || expression.arguments.is_some_and(|arguments| !arguments.nodes().is_empty())
    {
        return false;
    }
    tp.is_node_reference_to_effect_cause_module_api(expression.expression, "NoSuchElementError")
}
