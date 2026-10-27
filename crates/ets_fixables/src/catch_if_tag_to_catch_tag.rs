use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::ast::TokenFlags;
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_CATCH_TAG_EXPRESSES_TAGGED_ERROR_RECOVERY_MORE_DIRECTLY_THAN_EFFECT_CATCH_IF_WITH_A_TAG_EQUALITY_PREDICATE_EFFECT_CATCH_IF_TAG_TO_CATCH_TAG;

// go: internal/fixables/catch_if_tag_to_catch_tag.go:12 CatchIfTagToCatchTagFix
pub static CATCH_IF_TAG_TO_CATCH_TAG_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "catchIfTagToCatchTag",
    description: "Replace Effect.catchIf with Effect.catchTag",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["catchIfTagToCatchTag_fix"],
    run: run_catch_if_tag_to_catch_tag_fix,
});

// go: internal/fixables/catch_if_tag_to_catch_tag.go:18 runCatchIfTagToCatchTagFix
fn run_catch_if_tag_to_catch_tag_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_catch_if_tag_to_catch_tag(&mut ctx.type_parser, Some(source_file)) {
        if !m.can_fix {
            continue;
        }
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let action = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.catchTag".to_string(),
            run: Box::new(move |tracker| {
                let callee = effect_module_method(tracker, source_file, "catchTag");
                let arguments = tracker.new_node_list(vec![
                    tracker.new_string_literal(alloc_str(&m.tag), TokenFlags::None),
                    tracker.deep_clone_node(Some(m.handler)).unwrap(),
                ]);
                tracker.replace_piping_flow_transformation(
                    Some(source_file),
                    Some(&m.transformation),
                    PipingFlowTransformationReplacement {
                        callee: Some(callee),
                        type_arguments: None,
                        arguments: Some(arguments),
                    },
                );
            }),
        });
        if let Some(action) = action {
            return vec![action];
        }
    }
    Vec::new()
}
