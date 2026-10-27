use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{PipingFlowTransformationReplacement, Tracker};
use ets_rules::CatchToIgnoreMatch;
use ets_shim::P;
use ets_shim::ast::{self, Node, SourceFile};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_1_EXPRESSES_IGNORED_FAILURE_MORE_DIRECTLY_THAN_EFFECT_0_RETURNING_EFFECT_VOID_EFFECT_CATCH_TO_IGNORE;

// go: internal/fixables/catch_to_ignore.go:12 CatchToIgnoreFix
pub static CATCH_TO_IGNORE_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "catchToIgnore",
    description: "Replace Effect.catch/catchCause + Effect.void with Effect.ignore/ignoreCause",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["catchToIgnore_fix"],
    run: run_catch_to_ignore_fix,
});

// go: internal/fixables/catch_to_ignore.go:20 runCatchToIgnoreFix
//
// Rust: the match's transformation is never nil (a non-optional field), so Go's nil check drops.
fn run_catch_to_ignore_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_catch_to_ignore(&mut ctx.type_parser, sf);
    for m in matches {
        let diag_range = m.location;
        if !diag_range.intersects(ctx.span) && !ctx.span.contained_by(diag_range) {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: format!("Replace with Effect.{}", m.ignore_method_name),
            run: Box::new(move |tracker| {
                let ignore_access = catch_to_ignore_replacement_access(tracker, sf, &m);
                tracker.replace_piping_flow_transformation(
                    Some(sf),
                    Some(&m.transformation),
                    PipingFlowTransformationReplacement {
                        callee: Some(ignore_access),
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

// go: internal/fixables/catch_to_ignore.go:49 catchToIgnoreReplacementAccess
fn catch_to_ignore_replacement_access(
    tracker: &Tracker,
    sf: P<SourceFile>,
    m: &CatchToIgnoreMatch,
) -> P<Node> {
    let replacement = effect_module_method(tracker, sf, m.ignore_method_name);
    ast::set_parent_in_children(replacement);
    replacement
}
