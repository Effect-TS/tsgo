use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node};
use ets_shim::checker::TypeFlags;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, LazyExpressionFlags, PipingFlow, PipingFlowSubject,
    PipingFlowTransformation, TypeParser, parse_lazy_expression,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::USE_EFFECT_TAP_ERROR_TO_OBSERVE_THE_ERROR_AND_PRESERVE_THE_ORIGINAL_FAILURE_WITHOUT_HAVING_TO_RE_FAIL_IT_EFFECT_CATCH_REFAIL_TO_TAP_ERROR;

// go: internal/rules/catch_refail_to_tap_error.go:14 CatchRefailToTapError d483c153d29fdcf7
// CatchRefailToTapError recognizes observation followed by an unchanged refail.
pub static CATCH_REFAIL_TO_TAP_ERROR: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "catchRefailToTapError",
    group: "style",
    description: "Suggests Effect.tapError for catch handlers that sequence an effect and then re-fail the original error",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        if ctx.type_parser.supported_effect_version() != EffectMajorVersion::V4 {
            return Vec::new();
        }
        let mut diagnostics = Vec::new();
        let flows = ctx.type_parser.piping_flows(source_file, true);
        for flow in flows.as_deref().unwrap_or_default() {
            for (i, _) in flow.transformations().all() {
                let step = flow.transformations().at(i);
                if step.args().len() != 1
                    || !ctx
                        .type_parser
                        .is_node_reference_to_effect_module_api(step.callee(), "catch")
                {
                    continue;
                }
                let input_type =
                    PipingFlow::transformation_input_type(Some(flow), ctx.type_parser, i);
                let input = ctx.type_parser.strict_effect_type(input_type);
                let Some(input) = input else {
                    continue;
                };
                if input.e().flags().intersects(TypeFlags::Never)
                    || !catch_handler_refails_original_error(ctx.type_parser, *step.args().at(0))
                {
                    continue;
                }
                diagnostics.push(ctx.new_diagnostic(
                    source_file,
                    ets_shim::scanner::get_error_range_for_node(source_file, step.callee()),
                    MESSAGE,
                    &[],
                    &[],
                ));
            }
        }
        diagnostics
    },
});

// go: internal/rules/catch_refail_to_tap_error.go:52 catchHandlerRefailsOriginalError e1a402ea3623bbb3
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn catch_handler_refails_original_error(
    tp: &mut TypeParser<'_>,
    handler: P<Node>,
) -> bool {
    let Some(lazy) = parse_lazy_expression(handler, LazyExpressionFlags::None) else {
        return false;
    };
    if lazy.params().len() != 1 {
        return false;
    }
    let parameter_node = *lazy.params().at(0);
    let parameter = parameter_node.as_parameter_declaration();
    let Some(parameter_name) = parameter_node.name() else {
        return false;
    };
    if parameter_name.kind() != Kind::Identifier
        || parameter.initializer().is_some()
        || parameter.dot_dot_dot_token().is_some()
    {
        return false;
    }
    let Some(error_symbol) = tp.get_symbol_at_location(parameter_name) else {
        return false;
    };
    if tp.checker().is_symbol_assigned(error_symbol) {
        return false;
    }

    let flow = tp.longest_piping_flow_at(lazy.expression(), false);
    let Some(flow) = flow else {
        return false;
    };
    if flow.transformations().len() == 0 {
        return false;
    }
    let last = flow.transformations().len() - 1;
    let step = flow.transformations().at(last);
    if step.args().len() != 1 {
        return false;
    }
    let input_type = PipingFlow::transformation_input_type(Some(&flow), tp, last);
    if tp.strict_effect_type(input_type).is_none() {
        return false;
    }

    let mut refail = *step.args().at(0);
    if tp.is_node_reference_to_effect_module_api(step.callee(), "andThen") {
        // andThen accepts both an Effect and a callback. Only a zero-argument
        // callback can be discarded without binding the preceding success value.
        if let Some(thunk) = parse_lazy_expression(refail, LazyExpressionFlags::Thunk) {
            refail = thunk.expression();
        }
    } else if tp.is_node_reference_to_effect_module_api(step.callee(), "flatMap") {
        let Some(thunk) = parse_lazy_expression(refail, LazyExpressionFlags::Thunk) else {
            return false;
        };
        refail = thunk.expression();
    } else {
        return false;
    }

    // Matching the whole inner flow excludes recovery or other work after fail,
    // and excludes expressions which merely compute a different error value.
    let inner = tp.longest_piping_flow_at(refail, false);
    PipingFlow::matches_exactly(
        inner.as_ref(),
        tp,
        &|tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
            let node = subject.node().map(ast::skip_parentheses);
            node.is_some_and(|node| {
                node.kind() == Kind::Identifier
                    && tp.get_symbol_at_location(node) == Some(error_symbol)
            })
        },
        &[&|tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| {
            step.args().len() == 0
                && tp.is_node_reference_to_effect_module_api(step.callee(), "fail")
        }],
    )
}
