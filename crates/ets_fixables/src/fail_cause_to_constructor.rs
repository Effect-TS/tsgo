use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::Tracker;
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, NodeFlags, SourceFile};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::{PipingFlow, PipingFlowTransformation, TransformationKind, TypeParser};

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_EXPRESSES_THIS_FAILURE_DIRECTLY_INSTEAD_OF_WRAPPING_CAUSE_2_IN_EFFECT_1_EFFECT_FAIL_CAUSE_TO_CONSTRUCTOR;

// go: internal/fixables/fail_cause_to_constructor.go:14 FailCauseToConstructorFix 37d7ff7453c15089
pub static FAIL_CAUSE_TO_CONSTRUCTOR_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "failCauseToConstructor",
    description: "Replace with the direct Effect failure constructor",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["failCauseToConstructor_fix"],
    run: run_fail_cause_to_constructor_fix,
});

// go: internal/fixables/fail_cause_to_constructor.go:24 runFailCauseToConstructorFix c82caaa6cb41b598
fn run_fail_cause_to_constructor_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_fail_cause_to_constructor(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let Some(flow) = &m.flow else {
            continue;
        };
        if m.transformation_count <= 0 {
            continue;
        }
        let fail_step = flow.transformations().at(m.transformation_count - 1).clone();
        let edit = if m.lazy.is_none() {
            fail_cause_to_constructor_eager_edit(sf, &m, &fail_step)
        } else {
            fail_cause_to_constructor_lazy_edit(ctx, &m, &fail_step)
        };
        let Some(edit) = edit else {
            continue;
        };
        // The callee is rewritten through the file's Effect import, which must
        // reach the replacement API where the edit lands.
        let Some(member_name) = replacement_member_name(&m.replacement_name) else {
            continue;
        };
        let member = effect_module_member(member_name);
        let Some(target) = imported_module_member(
            ctx,
            fail_step.callee(),
            &ets_typeparser::find_effect_module_reference(Some(sf)),
            &member,
        ) else {
            continue;
        };

        let description = "Replace with Effect.".to_string() + &m.replacement_name;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                let callee = target.expression(tracker, &member);
                edit(tracker, callee);
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}

/// Rust-only: the edit closure Go returns as `func(*rewriter.Tracker, *ast.Node)`.
type FailCauseToConstructorEdit = Box<dyn FnOnce(&mut Tracker, P<Node>)>;

// go: internal/fixables/fail_cause_to_constructor.go:67 failCauseToConstructorEagerEdit 40558e951ce536f2
// failCauseToConstructorEagerEdit replaces the flow prefix up to and including
// Effect.failCause with a call of the replacement on the Cause constructor's
// value. It returns nil when the value is not a standalone node or the
// failCause step is not applied by a call or a pipe.
//
// Rust: the match's flow is cloned into the edit, which outlives the match.
fn fail_cause_to_constructor_eager_edit(
    sf: P<SourceFile>,
    m: &ets_rules::FailCauseToConstructorMatch,
    fail_step: &PipingFlowTransformation,
) -> Option<FailCauseToConstructorEdit> {
    let flow = m.flow.clone()?;
    let cause_index = m.transformation_count - 2;
    let value = PipingFlow::transformation_input_node(Some(&flow), cause_index)?;
    if fail_step.kind() != TransformationKind::Call
        && fail_step.kind() != TransformationKind::Pipe
        && fail_step.kind() != TransformationKind::Pipeable
    {
        return None;
    }
    let value_type_arguments = flow.transformations().at(cause_index).type_arguments();
    let transformation_count = m.transformation_count;
    Some(Box::new(move |tracker: &mut Tracker, callee: P<Node>| {
        let mut type_arguments = None;
        if let Some(value_type_arguments) = value_type_arguments
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
            tracker.new_node_list(vec![tracker.deep_clone_node(Some(value)).unwrap()]),
            NodeFlags::None,
        );
        tracker.replace_piping_flow_prefix(
            Some(sf),
            Some(&flow),
            transformation_count,
            Some(replacement),
        );
    }))
}

// go: internal/fixables/fail_cause_to_constructor.go:101 failCauseToConstructorLazyEdit 91f636c7f050a560
// failCauseToConstructorLazyEdit replaces the Cause constructor call returned
// by the failCauseSync thunk with its value and renames the failCauseSync
// callee. It returns nil when the thunk returns more than that call, the value
// is not a standalone node, the Cause call has type arguments, or failSync
// would infer another error type.
fn fail_cause_to_constructor_lazy_edit(
    ctx: &mut Context<'_, '_>,
    m: &ets_rules::FailCauseToConstructorMatch,
    fail_step: &PipingFlowTransformation,
) -> Option<FailCauseToConstructorEdit> {
    let (Some(lazy), Some(inner)) = (m.lazy, m.inner.as_ref()) else {
        return None;
    };
    let last_index = inner.transformations().len() - 1;
    let last = inner.transformations().at(last_index);
    let value = PipingFlow::transformation_input_node(Some(inner), last_index);
    let body = ast::skip_parentheses(lazy.expression());
    if fail_step.kind() != TransformationKind::Call
        || ets_rules::has_explicit_type_arguments(Some(last))
        || value.is_none()
        || body.kind() != Kind::CallExpression
        || body.as_call_expression().expression() != last.callee()
        || m.constructor_name != "die"
            && !lazy_fail_keeps_error_type(&mut ctx.type_parser, fail_step, value.unwrap())
    {
        return None;
    }
    let value = value.unwrap();
    let expression_body = !ast::is_block(lazy.body()) && lazy.expression() == body;
    let sf = ctx.source_file;
    let fail_callee = fail_step.callee();
    Some(Box::new(move |tracker: &mut Tracker, callee: P<Node>| {
        let mut replacement = tracker.deep_clone_node(Some(value)).unwrap();
        // An object literal as a concise arrow body would parse as a block.
        if expression_body && ast::is_object_literal_expression(ast::skip_parentheses(value)) {
            replacement = tracker.new_parenthesized_expression(replacement);
        }
        tracker.replace_node(sf, Some(body), Some(replacement), None);
        tracker.replace_node(sf, Some(fail_callee), Some(callee), None);
    }))
}

// go: internal/fixables/fail_cause_to_constructor.go:130 lazyFailKeepsErrorType 07e14f43ac97c387
// lazyFailKeepsErrorType reports whether Effect.failSync(() => value) infers the
// same error type as Effect.failCauseSync(() => Cause.fail(value)). Cause.fail
// infers its error from a call argument, which widens literal unions, while a
// failSync thunk returns the value against LazyArg<E>, which keeps them: for
// `cond ? "A" : "B"` the first infers string and the second "A" | "B". The fix
// is only offered when the value's own type already is the inferred error type.
//
// Rust: Go's checker argument is `tp.checker()`; Go's `effect.E() == nil` check drops (non-optional here).
fn lazy_fail_keeps_error_type(
    tp: &mut TypeParser<'_>,
    fail_step: &PipingFlowTransformation,
    value: P<Node>,
) -> bool {
    let out_type = fail_step.out_type(tp);
    let effect = tp.effect_type(out_type);
    let value_type = tp.get_type_at_location(value);
    let (Some(effect), Some(value_type)) = (effect, value_type) else {
        return false;
    };
    tp.checker().is_type_assignable_to(value_type, effect.e())
        && tp.checker().is_type_assignable_to(effect.e(), value_type)
}

/// Rust-only: `effect_module_member` takes a `&'static str` while the match
/// owns the replacement name.
fn replacement_member_name(name: &str) -> Option<&'static str> {
    ["fail", "die", "failSync", "dieSync"].into_iter().find(|candidate| *candidate == name)
}
