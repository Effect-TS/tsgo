use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's `Effect.isSuccess` message (Go spells it out in `ErrorCodes`).
static IS_SUCCESS_MESSAGE: &Message = &tsdiag::EFFECT_IS_SUCCESS_EXPRESSES_THIS_CHECK_MORE_DIRECTLY_THAN_EFFECT_0_FOLDING_TO_COMPLEMENTARY_BOOLEAN_LITERALS_EFFECT_MATCH_BOOLEAN_TO_IS_SUCCESS;

/// Rust-only: the fixable's `Effect.isFailure` message (Go spells it out in `ErrorCodes`).
static IS_FAILURE_MESSAGE: &Message = &tsdiag::EFFECT_IS_FAILURE_EXPRESSES_THIS_CHECK_MORE_DIRECTLY_THAN_EFFECT_0_FOLDING_TO_COMPLEMENTARY_BOOLEAN_LITERALS_EFFECT_MATCH_BOOLEAN_TO_IS_SUCCESS;

// go: internal/fixables/match_boolean_to_is_success.go:12 MatchBooleanToIsSuccessFix 698a4613178d49b7
pub static MATCH_BOOLEAN_TO_IS_SUCCESS_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "matchBooleanToIsSuccess",
    description: "Replace Effect.match boolean folds with Effect.isSuccess or Effect.isFailure",
    error_codes: vec![IS_SUCCESS_MESSAGE.code(), IS_FAILURE_MESSAGE.code()],
    fix_ids: vec!["matchBooleanToIsSuccess_fix"],
    run: run_match_boolean_to_is_success_fix,
});

// go: internal/fixables/match_boolean_to_is_success.go:23 runMatchBooleanToIsSuccessFix 87dff8cd719624c9
fn run_match_boolean_to_is_success_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_match_boolean_to_is_success(&mut ctx.type_parser, Some(source_file))
    {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let member = effect_module_member(m.replacement_name);
        let (reference, named_imports) = callee_module_member_reference(
            ctx,
            m.callee,
            &ets_typeparser::find_effect_module_reference(Some(source_file)),
            &member,
        );
        let Some(reference) = reference else {
            continue;
        };

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: format!("Replace with Effect.{}", m.replacement_name),
            run: Box::new(move |tracker| {
                let transformation = m.flow.transformations().at(m.index).clone();
                let replaced = tracker.replace_piping_flow_transformation(
                    Some(m.source_file),
                    Some(&transformation),
                    PipingFlowTransformationReplacement {
                        callee: Some(reference.expression(tracker, &member)),
                        type_arguments: None,
                        arguments: None,
                    },
                );
                if replaced && let Some(named_imports) = named_imports {
                    let elements = named_imports.as_named_imports().elements();
                    let specifier = tracker.new_import_specifier(
                        false,
                        None,
                        tracker.new_identifier(member.name),
                    );
                    tracker.insert_import_specifier_at_index(
                        m.source_file,
                        specifier,
                        named_imports,
                        elements.nodes().len(),
                    );
                }
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
