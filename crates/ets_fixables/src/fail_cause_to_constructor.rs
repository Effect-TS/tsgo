use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, NodeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_EXPRESSES_THIS_FAILURE_DIRECTLY_INSTEAD_OF_WRAPPING_CAUSE_2_IN_EFFECT_1_EFFECT_FAIL_CAUSE_TO_CONSTRUCTOR;

// go: internal/fixables/fail_cause_to_constructor.go:13 FailCauseToConstructorFix 37d7ff7453c15089
pub static FAIL_CAUSE_TO_CONSTRUCTOR_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "failCauseToConstructor",
    description: "Replace with the direct Effect failure constructor",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["failCauseToConstructor_fix"],
    run: run_fail_cause_to_constructor_fix,
});

// go: internal/fixables/fail_cause_to_constructor.go:23 runFailCauseToConstructorFix 9efc8105ebef4e9c
fn run_fail_cause_to_constructor_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_fail_cause_to_constructor(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if m.flow.is_none() && m.lazy_cause_node.is_none() {
            continue;
        }
        // The callee is rewritten through the file's Effect import, which must
        // reach the replacement API where the edit lands.
        let mut callee = m.lazy_callee;
        if let Some(flow) = &m.flow {
            callee = Some(flow.transformations().at(m.transformation_count - 1).callee());
        }
        let Some(member_name) = replacement_member_name(&m.replacement_name) else {
            continue;
        };
        let member = effect_module_member(member_name);
        let Some(target) = callee.and_then(|callee| {
            imported_module_member(
                ctx,
                callee,
                &ets_typeparser::find_effect_module_reference(Some(sf)),
                &member,
            )
        }) else {
            continue;
        };

        let description = "Replace with Effect.".to_string() + &m.replacement_name;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                if m.flow.is_some() {
                    let callee = target.expression(tracker, &member);
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
                let replacement = target.expression(tracker, &member);
                tracker.replace_node(sf, m.lazy_callee, Some(replacement), None);
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}

/// Rust-only: `effect_module_member` takes a `&'static str` while the match
/// owns the replacement name.
fn replacement_member_name(name: &str) -> Option<&'static str> {
    ["fail", "die", "failSync", "dieSync"].into_iter().find(|candidate| *candidate == name)
}
