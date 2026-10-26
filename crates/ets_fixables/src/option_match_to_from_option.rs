use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{PipingFlowTransformationReplacement, Tracker};
use ets_rules::OptionMatchToFromOptionMatch;
use ets_shim::ast::{self, Kind, Node, NodeFlags, NodeList};
use ets_shim::core::{P, alloc_str};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_FROM_OPTION_EXPRESSES_THIS_OPTION_TO_EFFECT_CONVERSION_MORE_DIRECTLY_THAN_OPTION_MATCH_OR_AN_OPTION_TAG_CONDITIONAL_EFFECT_OPTION_MATCH_TO_FROM_OPTION;

// go: internal/fixables/option_match_to_from_option.go:13 OptionMatchToFromOptionFix
pub static OPTION_MATCH_TO_FROM_OPTION_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "optionMatchToFromOption",
    description: "Replace Option.match or an Option tag conditional with Effect.fromOption",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["optionMatchToFromOption_fix"],
    run: run_option_match_to_from_option_fix,
});

// go: internal/fixables/option_match_to_from_option.go:23 runOptionMatchToFromOptionFix
fn run_option_match_to_from_option_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_option_match_to_from_option(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if !m.can_fix
            || m.transformation.is_none()
                && (m.replacement_node.is_none() || m.option_node.is_none())
        {
            continue;
        }
        let effect_module_name = ets_typeparser::find_effect_module_identifier(sf);

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.fromOption".to_string(),
            run: Box::new(move |tracker| {
                // Rust: Go returns early on a nil callee; `build_from_option_replacement` never returns one.
                let (callee, arguments) =
                    build_from_option_replacement(tracker, &m, &effect_module_name);
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

// go: internal/fixables/option_match_to_from_option.go:64 buildFromOptionReplacement
pub(crate) fn build_from_option_replacement(
    tracker: &Tracker,
    m: &OptionMatchToFromOptionMatch,
    effect_module_name: &str,
) -> (P<Node>, Option<P<NodeList>>) {
    let module_node = match m.effect_module_node {
        None => tracker.new_identifier(alloc_str(effect_module_name)),
        Some(module_node) => tracker.deep_clone_node(Some(module_node)).unwrap(),
    };
    let from_option = tracker.new_property_access_expression(
        module_node,
        None,
        tracker.new_identifier("fromOption"),
        NodeFlags::None,
    );

    if m.default_failure {
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
