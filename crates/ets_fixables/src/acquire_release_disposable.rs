use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, NodeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_ACQUIRE_DISPOSABLE_EXPRESSES_THIS_DISPOSABLE_RESOURCE_ACQUISITION_MORE_DIRECTLY_THAN_EFFECT_ACQUIRE_RELEASE_WITH_A_MANUAL_DISPOSAL_FINALIZER_EFFECT_ACQUIRE_RELEASE_DISPOSABLE;

// go: internal/fixables/acquire_release_disposable.go:12 AcquireReleaseDisposableFix
pub static ACQUIRE_RELEASE_DISPOSABLE_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "acquireReleaseDisposable",
    description: "Replace Effect.acquireRelease with Effect.acquireDisposable",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["acquireReleaseDisposable_fix"],
    run: run_acquire_release_disposable_fix,
});

// go: internal/fixables/acquire_release_disposable.go:22 runAcquireReleaseDisposableFix
fn run_acquire_release_disposable_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_acquire_release_disposable(&mut ctx.type_parser, Some(source_file))
    {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if m.has_type_arguments {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.acquireDisposable".to_string(),
            run: Box::new(move |tracker| {
                let callee = effect_module_method(tracker, source_file, "acquireDisposable");
                let replacement = tracker.new_call_expression(
                    callee,
                    None,
                    None,
                    tracker.new_node_list(vec![tracker.deep_clone_node(Some(m.acquire)).unwrap()]),
                    NodeFlags::None,
                );
                ast::set_parent_in_children(replacement);
                tracker.replace_node(source_file, Some(m.call_node), Some(replacement), None);
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
