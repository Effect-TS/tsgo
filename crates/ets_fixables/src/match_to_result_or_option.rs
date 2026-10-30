use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_REIFIES_THE_OUTCOME_DIRECTLY_THESE_EFFECT_1_HANDLERS_ONLY_WRAP_THE_VALUE_AND_THE_ERROR_EFFECT_MATCH_TO_RESULT_OR_OPTION;

// go: internal/fixables/match_to_result_or_option.go:12 MatchToResultOrOptionFix 3b30c1db34fbbcb0
pub static MATCH_TO_RESULT_OR_OPTION_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "matchToResultOrOption",
    description: "Replace with Effect.result, Effect.option or Effect.either",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["matchToResultOrOption_fix"],
    run: run_match_to_result_or_option_fix,
});

// go: internal/fixables/match_to_result_or_option.go:22 runMatchToResultOrOptionFix b4f66794605928e9
fn run_match_to_result_or_option_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_match_to_result_or_option(&mut ctx.type_parser, Some(source_file)) {
        if !m.fixable || (!m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location)) {
            continue;
        }
        let member = effect_module_member(m.replacement_name);
        let location = m.flow.transformations().at(m.end).callee();
        let target = resolve_module_member(
            ctx,
            location,
            &ets_typeparser::find_effect_module_reference(Some(m.source_file)),
            &member,
        );
        let Some(target) = target else {
            continue;
        };
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.".to_string() + m.replacement_name,
            run: Box::new(move |t| {
                let replacement = PipingFlowTransformationReplacement {
                    callee: Some(target.expression(t, &member)),
                    type_arguments: None,
                    arguments: None,
                };
                if m.start == m.end {
                    let step = m.flow.transformations().at(m.start);
                    t.replace_piping_flow_transformation(
                        Some(m.source_file),
                        Some(step),
                        replacement,
                    );
                    return;
                }
                t.collapse_piping_flow_transformations(
                    Some(m.source_file),
                    Some(&m.flow),
                    m.start,
                    m.end,
                    replacement,
                );
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
