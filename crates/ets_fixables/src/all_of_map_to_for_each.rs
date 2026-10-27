use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, NodeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_FOR_EACH_EXPRESSES_THIS_EFFECTFUL_ARRAY_MAPPING_MORE_DIRECTLY_THAN_EFFECT_ALL_OVER_ARRAY_MAP_EFFECT_ALL_OF_MAP_TO_FOR_EACH;

// go: internal/fixables/all_of_map_to_for_each.go:12 AllOfMapToForEachFix
pub static ALL_OF_MAP_TO_FOR_EACH_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "allOfMapToForEach",
    description: "Replace Effect.all over Array#map with Effect.forEach",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["allOfMapToForEach_fix"],
    run: run_all_of_map_to_for_each_fix,
});

// go: internal/fixables/all_of_map_to_for_each.go:22 runAllOfMapToForEachFix
fn run_all_of_map_to_for_each_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_all_of_map_to_for_each(&mut ctx.type_parser, source_file) {
        if !m.can_fix {
            continue;
        }
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        // The generic parameter lists of Array#map, Effect.all, and Effect.forEach
        // differ, so an explicit type argument cannot be moved safely.
        if m.has_type_arguments {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.forEach".to_string(),
            run: Box::new(move |tracker| {
                let callee = effect_module_method(tracker, source_file, "forEach");
                let mut arguments = vec![
                    tracker.deep_clone_node(Some(m.receiver)).unwrap(),
                    tracker.deep_clone_node(Some(m.callback)).unwrap(),
                ];
                if let Some(options) = m.options {
                    arguments.push(tracker.deep_clone_node(Some(options)).unwrap());
                }
                let replacement = tracker.new_call_expression(
                    callee,
                    None,
                    None,
                    tracker.new_node_list(arguments),
                    NodeFlags::None,
                );
                ast::set_parent_in_children(replacement);
                tracker.replace_node(source_file, m.call_node, Some(replacement), None);
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
