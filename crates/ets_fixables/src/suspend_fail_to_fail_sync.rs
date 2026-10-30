use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{PipingFlowTransformationReplacement, Tracker};
use ets_shim::P;
use ets_shim::ast::{self, Node, NodeFlags, SourceFile};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::TransformationKind;

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_BUILDS_THIS_1_LAZILY_DIRECTLY_INSTEAD_OF_WRAPPING_EFFECT_2_IN_EFFECT_SUSPEND_EFFECT_SUSPEND_FAIL_TO_FAIL_SYNC;

// go: internal/fixables/suspend_fail_to_fail_sync.go:13 SuspendFailToFailSyncFix f40ecc3c37a0909a
pub static SUSPEND_FAIL_TO_FAIL_SYNC_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "suspendFailToFailSync",
    description: "Replace with Effect.failSync or Effect.dieSync",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["suspendFailToFailSync_fix"],
    run: run_suspend_fail_to_fail_sync_fix,
});

// go: internal/fixables/suspend_fail_to_fail_sync.go:23 runSuspendFailToFailSyncFix 4edc31ca92710d3a
fn run_suspend_fail_to_fail_sync_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_suspend_fail_to_fail_sync(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if !m.fixable {
            continue;
        }
        let member = effect_module_member(m.replacement_name);
        let suspend_callee = m.suspend.callee();
        // Keep the qualifier of Effect.suspend when it is the Effect module, which
        // already resolves where the edit lands; otherwise go through the file's
        // Effect import.
        let mut qualifier: Option<P<Node>> = None;
        let mut target: Option<ModuleMemberReference> = None;
        if ast::is_property_access_expression(suspend_callee) {
            let expression = suspend_callee.as_property_access_expression().expression();
            if ctx.type_parser.is_expression_effect_module(expression) {
                let expression_type = ctx.type_parser.get_type_at_location(expression);
                if type_has_member(ctx, expression_type, &member) {
                    qualifier = Some(expression);
                }
            }
        }
        if qualifier.is_none() {
            target = imported_module_member(
                ctx,
                suspend_callee,
                &ets_typeparser::find_effect_module_reference(Some(sf)),
                &member,
            );
            if target.is_none() {
                continue;
            }
        }

        let description = "Replace with Effect.".to_string() + m.replacement_name;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                if !replace_suspend_thunk_body(tracker, sf, &m) {
                    return;
                }

                let mut callee = if let Some(qualifier) = qualifier {
                    tracker.new_property_access_expression(
                        tracker.deep_clone_node(Some(qualifier)).unwrap(),
                        None,
                        tracker.new_identifier(m.replacement_name),
                        NodeFlags::None,
                    )
                } else {
                    target.as_ref().unwrap().expression(tracker, &member)
                };
                if let Some(value_type_arguments) = m.value_type_arguments
                    && !value_type_arguments.nodes().is_empty()
                {
                    let arguments = value_type_arguments
                        .nodes()
                        .iter()
                        .map(|&argument| tracker.deep_clone_node(Some(argument)).unwrap())
                        .collect();
                    callee = tracker.new_expression_with_type_arguments(
                        callee,
                        Some(tracker.new_node_list(arguments)),
                    );
                }

                if m.suspend.kind() == TransformationKind::Call {
                    // The step replacement would reprint the whole suspend(thunk) call,
                    // overlapping the edit of the thunk body, so only the callee is
                    // replaced.
                    tracker.replace_node(sf, Some(suspend_callee), Some(callee), None);
                    return;
                }
                tracker.replace_piping_flow_transformation(
                    Some(sf),
                    Some(&m.suspend),
                    PipingFlowTransformationReplacement {
                        callee: Some(callee),
                        type_arguments: None,
                        arguments: None,
                    },
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}

// go: internal/fixables/suspend_fail_to_fail_sync.go:92 replaceSuspendThunkBody d41f79a07068351a
// replaceSuspendThunkBody makes the thunk return the error value instead of
// failing with it.
fn replace_suspend_thunk_body(
    tracker: &mut Tracker,
    sf: P<SourceFile>,
    m: &ets_rules::SuspendFailToFailSyncMatch,
) -> bool {
    let returned = m.thunk.expression();
    let concise_body = !ast::is_block(m.thunk.body());
    // Removing the step keeps its input as written, which cannot drop the
    // parentheses around the returned flow or wrap an object literal that a
    // concise arrow body would parse as a block, so those replace the returned
    // expression with the value.
    if let Some(value) = m.value
        && (returned != m.inner.node()
            || concise_body && ast::is_object_literal_expression(ast::skip_parentheses(value)))
    {
        let mut replacement = tracker.deep_clone_node(Some(value)).unwrap();
        if concise_body && ast::is_object_literal_expression(ast::skip_parentheses(value)) {
            replacement = tracker.new_parenthesized_expression(replacement);
        }
        tracker.replace_node(sf, Some(returned), Some(replacement), None);
        return true;
    }
    tracker.remove_trailing_piping_flow_transformation(Some(sf), Some(&m.inner))
}
