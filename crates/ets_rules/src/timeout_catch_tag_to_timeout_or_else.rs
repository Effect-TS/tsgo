use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, ModifierFlags, Node, SourceFile};
use ets_shim::checker::{LiteralValue, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{
    EffectMajorVersion, LazyExpressionFlags, PipingFlow, PipingFlowSubject,
    PipingFlowTransformation, TypeParser,
};

use crate::piping_flow::has_explicit_type_arguments;
use crate::prefer_succeed_some_or_none::is_option_none_call;

// go: internal/rules/timeout_catch_tag_to_timeout_or_else.go:14 TimeoutCatchTagToTimeoutOrElse a25e7ccc97e26a13
pub static TIMEOUT_CATCH_TAG_TO_TIMEOUT_OR_ELSE: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "timeoutCatchTagToTimeoutOrElse",
    group: "style",
    description: "Suggests dedicated timeout combinators instead of catching TimeoutError immediately after Effect.timeout",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![
        tsdiag::USE_EFFECT_0_TO_HANDLE_THIS_TIMEOUT_DIRECTLY_EFFECT_TIMEOUT_CATCH_TAG_TO_TIMEOUT_OR_ELSE
            .code(),
    ],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let source_file = ctx.source_file;
        for m in analyze_timeout_catch_tag_to_timeout_or_else(ctx.type_parser, Some(source_file)) {
            diagnostics.push(ctx.new_diagnostic(
                ctx.source_file,
                m.location,
                &tsdiag::USE_EFFECT_0_TO_HANDLE_THIS_TIMEOUT_DIRECTLY_EFFECT_TIMEOUT_CATCH_TAG_TO_TIMEOUT_OR_ELSE,
                &[],
                &[m.replacement_name],
            ));
        }
        diagnostics
    },
}
});

// go: internal/rules/timeout_catch_tag_to_timeout_or_else.go:30 TimeoutCatchTagToTimeoutOrElseMatch 07fc522439c5769d
//
// Rust: Go's `*typeparser.PipingFlow` (into the parser's cached flows) is a clone of the flow.
#[derive(Clone)]
pub struct TimeoutCatchTagToTimeoutOrElseMatch {
    pub flow: PipingFlow,
    pub start: i64,
    pub end: i64,
    pub location: TextRange,
    pub replacement_name: &'static str,
    pub duration: P<Node>,
    pub handler: P<Node>,
}

// go: internal/rules/timeout_catch_tag_to_timeout_or_else.go:38 AnalyzeTimeoutCatchTagToTimeoutOrElse 10c99fb4a5adeda3
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); the parser and checker are never nil.
pub fn analyze_timeout_catch_tag_to_timeout_or_else(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<TimeoutCatchTagToTimeoutOrElseMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();
    // Check both alternatives in one pass: collecting and sorting matches per
    // pattern adds work and allocations to this frequently run analysis.
    let flows = tp.piping_flows(sf, true);
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        let transformations = flow.transformations();
        let mut i: i64 = 0;
        while i + 1 < transformations.len() {
            let timeout = transformations.at(i);
            if timeout.args().len() != 1
                || !tp.is_node_reference_to_effect_module_api(timeout.callee(), "timeout")
            {
                i += 1;
                continue;
            }
            let mut end = i + 1;
            let option = timeout_as_some(tp, transformations.at(end));
            if option {
                end += 1;
            }
            if end >= transformations.len() {
                i += 1;
                continue;
            }
            let catch = transformations.at(end);
            if catch.args().len() != 2
                || !tp.is_node_reference_to_effect_module_api(catch.callee(), "catchTag")
            {
                i += 1;
                continue;
            }
            let tag = ast::skip_parentheses(*catch.args().at(0));
            if tag.kind() != Kind::StringLiteral || tag.text() != "TimeoutError" {
                i += 1;
                continue;
            }
            let mut has_type_args = false;
            for j in i..=end {
                has_type_args =
                    has_type_args || has_explicit_type_arguments(Some(transformations.at(j)));
            }
            if has_type_args {
                i += 1;
                continue;
            }
            let handler = ast::skip_parentheses(*catch.args().at(1));
            if !timeout_ignores_error(tp, Some(handler)) {
                i += 1;
                continue;
            }
            if option && !timeout_returns_none(tp, handler) {
                i += 1;
                continue;
            }
            let input_type = PipingFlow::transformation_input_type(Some(flow), tp, i);
            let input = tp.strict_effect_type(input_type);
            let Some(input) = input else {
                i += 1;
                continue;
            };
            if !timeout_error_excluded(tp, Some(input.e())) {
                i += 1;
                continue;
            }
            let mut name = "timeoutOrElse";
            if option {
                name = "timeoutOption";
            }
            matches.push(TimeoutCatchTagToTimeoutOrElseMatch {
                flow: flow.clone(),
                start: i,
                end,
                location: ets_shim::scanner::get_error_range_for_node(sf, catch.callee()),
                replacement_name: name,
                duration: *timeout.args().at(0),
                handler: *catch.args().at(1),
            });
            i += 1;
        }
    }
    matches
}

// go: internal/rules/timeout_catch_tag_to_timeout_or_else.go:95 timeoutAsSome 94c0859b4ab529c6
pub(crate) fn timeout_as_some(tp: &mut TypeParser<'_>, step: &PipingFlowTransformation) -> bool {
    step.args().len() == 0 && tp.is_node_reference_to_effect_module_api(step.callee(), "asSome")
        || step.args().len() == 1
            && tp.is_node_reference_to_effect_module_api(step.callee(), "map")
            && tp.is_node_reference_to_effect_option_module_api(*step.args().at(0), "some")
}

// go: internal/rules/timeout_catch_tag_to_timeout_or_else.go:100 timeoutReturnsNone e7728900673c6a3a
pub(crate) fn timeout_returns_none(tp: &mut TypeParser<'_>, handler: P<Node>) -> bool {
    let Some(lazy) = ets_typeparser::parse_lazy_expression(handler, LazyExpressionFlags::None)
    else {
        return false;
    };
    let flow = tp.longest_piping_flow_at(lazy.expression(), false);
    PipingFlow::matches_exactly(
        flow.as_ref(),
        tp,
        &|tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
            subject
                .node()
                .is_some_and(|node| tp.is_node_reference_to_effect_module_api(node, "succeedNone"))
        },
        &[],
    ) || PipingFlow::matches_exactly(
        flow.as_ref(),
        tp,
        &|tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
            is_option_none_call(tp, subject.node())
        },
        &[&|tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| {
            step.args().len() == 0
                && step
                    .type_arguments()
                    .is_none_or(|type_arguments| type_arguments.nodes().is_empty())
                && tp.is_node_reference_to_effect_module_api(step.callee(), "succeed")
        }],
    )
}

// go: internal/rules/timeout_catch_tag_to_timeout_or_else.go:121 timeoutErrorExcluded c039a64d005e6e1e
// Fail closed for unknown, generic, or broadly tagged errors: catchTag matches
// structurally by tag, so checking only the canonical TimeoutError is unsafe.
pub(crate) fn timeout_error_excluded(tp: &mut TypeParser<'_>, error_type: Option<P<Type>>) -> bool {
    let Some(error_type) = error_type else {
        return false;
    };
    if error_type.flags().intersects(TypeFlags::Never) {
        return true;
    }
    for member in tp.unroll_union_members(error_type).unwrap_or_default() {
        let Some(tag) = tp.checker().get_type_of_property_of_type_exported(member, "_tag") else {
            return false;
        };
        for value in tp.unroll_union_members(tag).unwrap_or_default() {
            if !value.flags().intersects(TypeFlags::StringLiteral) {
                return false;
            }
            match value.as_literal_type().value() {
                Some(LiteralValue::String(text)) if text != "TimeoutError" => {}
                _ => return false,
            }
        }
    }
    true
}

// go: internal/rules/timeout_catch_tag_to_timeout_or_else.go:147 timeoutIgnoresError fc8dd5d22bae030c
// Restrict callbacks to arrows so the caught error cannot be observed through
// a function's arguments object. An unused plain parameter can be removed.
pub(crate) fn timeout_ignores_error(tp: &mut TypeParser<'_>, handler: Option<P<Node>>) -> bool {
    let Some(handler) = handler else {
        return false;
    };
    if handler.kind() != Kind::ArrowFunction
        || ast::get_combined_modifier_flags(handler).intersects(ModifierFlags::Async)
    {
        return false;
    }
    if let Some(params) = ets_typeparser::get_function_like_type_parameters(handler)
        && !params.nodes().is_empty()
    {
        return false;
    }
    let params = ets_typeparser::get_function_like_parameters(handler);
    let Some(params) = params.filter(|params| !params.nodes().is_empty()) else {
        return true;
    };
    if params.nodes().len() != 1 {
        return false;
    }
    let parameter = params.nodes()[0].as_parameter_declaration();
    if parameter.name().kind() != Kind::Identifier
        || parameter.initializer().is_some()
        || parameter.dot_dot_dot_token().is_some()
    {
        return false;
    }
    let Some(symbol) = tp.get_symbol_at_location(parameter.name()) else {
        return false;
    };

    // Rust: Go's recursive `usesParameter` closure is a nested function.
    fn uses_parameter(
        tp: &mut TypeParser<'_>,
        symbol: P<ets_shim::ast::Symbol>,
        node: P<Node>,
    ) -> bool {
        if node.kind() == Kind::ShorthandPropertyAssignment
            && tp.checker().get_shorthand_assignment_value_symbol(Some(node)) == Some(symbol)
        {
            return true;
        }
        if node.kind() == Kind::Identifier && tp.get_symbol_at_location(node) == Some(symbol) {
            return true;
        }
        node.for_each_child(&mut |child| uses_parameter(tp, symbol, child))
    }

    let arrow = handler.as_arrow_function();
    let body = handler.body().expect("an arrow function has a body");
    !uses_parameter(tp, symbol, body)
        && arrow.type_().is_none_or(|type_node| !uses_parameter(tp, symbol, type_node))
}
