use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{PipingFlowTransformationReplacement, Tracker};
use ets_rules::OptionMatchToFromOptionMatch;
use ets_shim::ast::{self, Kind, Node, NodeFlags, NodeList, SourceFile};
use ets_shim::core::P;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::TypeParser;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_FROM_OPTION_EXPRESSES_THIS_OPTION_TO_EFFECT_CONVERSION_MORE_DIRECTLY_THAN_OPTION_MATCH_OR_AN_OPTION_TAG_CONDITIONAL_EFFECT_OPTION_MATCH_TO_FROM_OPTION;

// go: internal/fixables/option_match_to_from_option.go:13 OptionMatchToFromOptionFix 018122f1be22165e
pub static OPTION_MATCH_TO_FROM_OPTION_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "optionMatchToFromOption",
    description: "Replace Option.match or an Option tag conditional with Effect.fromOption",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["optionMatchToFromOption_fix"],
    run: run_option_match_to_from_option_fix,
});

// go: internal/fixables/option_match_to_from_option.go:23 runOptionMatchToFromOptionFix 346a33fd6fa5fc16
//
// Rust: Go's edit closure asks the type parser whether the failure is the default NoSuchElementError. The parser is
// borrowed mutably by the context, which `new_fix_action` borrows, so that check (which reads only the source and
// the checker, never the tracker) runs just before `new_fix_action`.
fn run_option_match_to_from_option_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_option_match_to_from_option(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        // A function expression thunk can read its own this or arguments, which
        // the arrow function the fix writes would not bind.
        if m.failure_handler.is_some_and(|handler| handler.kind() != Kind::ArrowFunction)
            || m.transformation.is_none()
                && (m.replacement_node.is_none() || m.option_node.is_none())
        {
            continue;
        }
        let default_failure =
            is_default_no_such_element_error(&mut ctx.type_parser, m.failure_node);
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.fromOption".to_string(),
            run: Box::new(move |tracker| {
                // Rust: Go returns early on a nil callee; `build_from_option_replacement` never returns one.
                let (callee, arguments) =
                    build_from_option_replacement(tracker, default_failure, sf, &m);
                if let Some(transformation) = &m.transformation {
                    tracker.replace_piping_flow_transformation(
                        Some(sf),
                        Some(transformation),
                        PipingFlowTransformationReplacement {
                            callee: Some(callee),
                            arguments,
                            ..Default::default()
                        },
                    );
                    return;
                }

                let mut direct_arguments = vec![tracker.deep_clone_node(m.option_node).unwrap()];
                if let Some(arguments) = arguments {
                    direct_arguments.extend_from_slice(arguments.nodes());
                }
                let replacement = tracker.new_call_expression(
                    callee,
                    None,
                    None,
                    tracker.new_node_list(direct_arguments),
                    NodeFlags::None,
                );
                ast::set_parent_in_children(replacement);
                tracker.replace_node(sf, m.replacement_node, Some(replacement), None);
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/option_match_to_from_option.go:64 buildFromOptionReplacement f4baf23a820ac112
//
// Rust: Go's type parser parameter is `default_failure`, the precomputed `isDefaultNoSuchElementError` result (see
// `run_option_match_to_from_option_fix`).
pub(crate) fn build_from_option_replacement(
    tracker: &Tracker,
    default_failure: bool,
    sf: P<SourceFile>,
    m: &OptionMatchToFromOptionMatch,
) -> (P<Node>, Option<P<NodeList>>) {
    let from_option = tracker.new_property_access_path(
        &ets_typeparser::find_effect_module_reference(Some(sf)).append(&["fromOption"]),
    );

    // Effect.fromOption fails with a new NoSuchElementError by default.
    if default_failure {
        return (from_option, None);
    }

    (
        from_option,
        Some(tracker.new_node_list(vec![tracker.new_arrow_function(
            None,
            None,
            Some(tracker.new_node_list(Vec::new())),
            None,
            None,
            Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
            tracker.deep_clone_node(m.failure_node),
        )])),
    )
}

// go: internal/fixables/option_match_to_from_option.go:85 isDefaultNoSuchElementError 760fc7ce925cd416
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
