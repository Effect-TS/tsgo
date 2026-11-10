use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{PipingFlowTransformationReplacement, Tracker};
use ets_shim::P;
use ets_shim::ast::{self, Node, NodeFlags, SourceFile};
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::{PipingFlow, PipingFlowTransformation, TransformationKind, TypeParser};

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_BUILDS_THIS_1_LAZILY_DIRECTLY_INSTEAD_OF_WRAPPING_EFFECT_2_IN_EFFECT_SUSPEND_EFFECT_SUSPEND_FAIL_TO_FAIL_SYNC;

// go: internal/fixables/suspend_fail_to_fail_sync.go:14 SuspendFailToFailSyncFix f40ecc3c37a0909a
pub static SUSPEND_FAIL_TO_FAIL_SYNC_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "suspendFailToFailSync",
    description: "Replace with Effect.failSync or Effect.dieSync",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["suspendFailToFailSync_fix"],
    run: run_suspend_fail_to_fail_sync_fix,
});

// go: internal/fixables/suspend_fail_to_fail_sync.go:24 runSuspendFailToFailSyncFix dcd0ea8cb00ca674
fn run_suspend_fail_to_fail_sync_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_suspend_fail_to_fail_sync(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let last_index = m.inner.transformations().len() - 1;
        let last = m.inner.transformations().at(last_index).clone();
        let value_type_arguments = last.type_arguments();
        let has_value_type_arguments =
            value_type_arguments.is_some_and(|arguments| !arguments.nodes().is_empty());
        // Explicit type arguments pin the error type in both forms.
        if m.inner_name == "fail" && !has_value_type_arguments && {
            let value_type = PipingFlow::transformation_input_type(
                Some(&m.inner),
                &mut ctx.type_parser,
                last_index,
            );
            !fail_sync_keeps_error_type(&mut ctx.type_parser, &last, value_type)
        } {
            continue;
        }
        let value = PipingFlow::transformation_input_node(Some(&m.inner), last_index);
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
                if !replace_suspend_thunk_body(tracker, sf, &m, value) {
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
                if has_value_type_arguments {
                    let value_type_arguments = value_type_arguments.unwrap();
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

// go: internal/fixables/suspend_fail_to_fail_sync.go:101 replaceSuspendThunkBody 8ec0452b4b50b6b0
// replaceSuspendThunkBody makes the thunk return the error value instead of
// failing with it. value is the input of the inner flow's last step, when it is
// a standalone node.
fn replace_suspend_thunk_body(
    tracker: &mut Tracker,
    sf: P<SourceFile>,
    m: &ets_rules::SuspendFailToFailSyncMatch,
    value: Option<P<Node>>,
) -> bool {
    let returned = m.thunk.expression();
    let concise_body = !ast::is_block(m.thunk.body());
    // Removing the step keeps its input as written, which cannot drop the
    // parentheses around the returned flow or wrap an object literal that a
    // concise arrow body would parse as a block, so those replace the returned
    // expression with the value.
    if let Some(value) = value
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

// go: internal/fixables/suspend_fail_to_fail_sync.go:127 failSyncKeepsErrorType 4f4ec1b16742f68e
// failSyncKeepsErrorType reports whether Effect.failSync(() => value) infers the
// same error type as Effect.fail(value). Effect.fail infers its error from a
// call argument, which widens literal unions, while a failSync thunk returns the
// value against LazyArg<E>, which keeps them: for `cond ? "A" : "B"` the first
// infers string and the second "A" | "B". A single literal widens in both forms.
// The input type of a flow step is a regular literal type, which only the base
// type of a literal widens.
//
// Rust: Go's checker argument is the parser's checker, so it is dropped. Go's `effect.E() == nil` check drops:
// `Effect::e` is non-optional.
fn fail_sync_keeps_error_type(
    tp: &mut TypeParser<'_>,
    fail_step: &PipingFlowTransformation,
    value_type: Option<P<Type>>,
) -> bool {
    let out_type = fail_step.out_type(tp);
    let effect = tp.effect_type(out_type);
    let (Some(effect), Some(value_type)) = (effect, value_type) else {
        return false;
    };
    let same_type = |tp: &mut TypeParser<'_>, t: P<Type>| -> bool {
        tp.checker().is_type_assignable_to(t, effect.e())
            && tp.checker().is_type_assignable_to(effect.e(), t)
    };
    same_type(tp, value_type)
        || value_type.flags().intersects(TypeFlags::Unit) && {
            let base = tp.checker().get_base_type_of_literal_type_exported(value_type);
            same_type(tp, base)
        }
}
