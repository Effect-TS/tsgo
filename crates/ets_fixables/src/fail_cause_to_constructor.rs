use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, Kind, NodeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_EXPRESSES_THIS_FAILURE_DIRECTLY_INSTEAD_OF_WRAPPING_CAUSE_2_IN_EFFECT_1_EFFECT_FAIL_CAUSE_TO_CONSTRUCTOR;

// go: internal/fixables/fail_cause_to_constructor.go:12 FailCauseToConstructorFix
pub static FAIL_CAUSE_TO_CONSTRUCTOR_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "failCauseToConstructor",
    description: "Replace with the direct Effect failure constructor",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["failCauseToConstructor_fix"],
    run: run_fail_cause_to_constructor_fix,
});

// go: internal/fixables/fail_cause_to_constructor.go:22 runFailCauseToConstructorFix
fn run_fail_cause_to_constructor_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_fail_cause_to_constructor(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if m.flow.is_none() && m.lazy_cause_node.is_none() {
            continue;
        }

        let description = "Replace with Effect.".to_string() + &m.replacement_name;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                if m.flow.is_some() {
                    let callee = effect_module_method(
                        tracker,
                        sf,
                        m.effect_module_node,
                        &m.replacement_name,
                    );
                    let mut type_arguments = None;
                    if let Some(value_type_arguments) = m.value_type_arguments
                        && !value_type_arguments.nodes().is_empty()
                    {
                        let arguments = value_type_arguments
                            .nodes()
                            .iter()
                            .map(|&argument| tracker.deep_clone_node(Some(argument)).unwrap())
                            .collect();
                        type_arguments = Some(tracker.new_node_list(arguments));
                    }
                    let replacement = tracker.new_call_expression(
                        callee,
                        None,
                        type_arguments,
                        tracker.new_node_list(vec![tracker.deep_clone_node(m.value_node).unwrap()]),
                        NodeFlags::None,
                    );
                    tracker.replace_piping_flow_prefix(
                        Some(sf),
                        m.flow.as_ref(),
                        m.transformation_count,
                        Some(replacement),
                    );
                    return;
                }

                let mut value = tracker.deep_clone_node(m.value_node).unwrap();
                // An object literal as a concise arrow body would parse as a block.
                if m.lazy_expression_body
                    && ast::is_object_literal_expression(ast::skip_parentheses(
                        m.value_node.unwrap(),
                    ))
                {
                    value = tracker.new_parenthesized_expression(value);
                }
                tracker.replace_node(sf, m.lazy_cause_node, Some(value), None);
                let lazy_callee = m.lazy_callee.unwrap();
                let mut name = None;
                if lazy_callee.kind() == Kind::PropertyAccessExpression {
                    name = Some(lazy_callee.as_property_access_expression().name());
                }
                replace_effect_method_callee(
                    tracker,
                    sf,
                    Some(lazy_callee),
                    name,
                    &m.replacement_name,
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
