use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rules::ZipViaNestedFlatMapToAllOperand;
use ets_shim::P;
use ets_shim::ast::{Node, NodeFactory, NodeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_NESTED_EFFECT_FLAT_MAP_SLASH_EFFECT_MAP_ONLY_PAIRS_TWO_INDEPENDENT_EFFECTS_USE_EFFECT_ALL_SEQUENTIAL_BY_DEFAULT_TO_EXPRESS_THIS_DIRECTLY_EFFECT_ZIP_VIA_NESTED_FLAT_MAP_TO_ALL;

// go: internal/fixables/zip_via_nested_flat_map_to_all.go:13 ZipViaNestedFlatMapToAllFix c91a74700b696fd1
pub static ZIP_VIA_NESTED_FLAT_MAP_TO_ALL_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "zipViaNestedFlatMapToAll",
    description: "Replace with Effect.all",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["zipViaNestedFlatMapToAll_fix"],
    run: run_zip_via_nested_flat_map_to_all_fix,
});

// go: internal/fixables/zip_via_nested_flat_map_to_all.go:23 runZipViaNestedFlatMapToAllFix 239c3d10eaaf992a
fn run_zip_via_nested_flat_map_to_all_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_zip_via_nested_flat_map_to_all(&mut ctx.type_parser, source_file) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if !m.fixable {
            continue;
        }
        let all = effect_module_member("all");
        let all_reference = resolve_module_member(
            ctx,
            m.callee,
            &ets_typeparser::find_effect_module_reference(Some(source_file)),
            &all,
        );
        let Some(all_reference) = all_reference else {
            continue;
        };

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.all".to_string(),
            run: Box::new(move |tracker| {
                let a = zip_operand_expression(tracker, m.a);
                let b = zip_operand_expression(tracker, m.b);
                let argument = if m.record {
                    tracker.new_object_literal_expression(
                        tracker.new_node_list(vec![
                            tracker.new_property_assignment(
                                None,
                                tracker.deep_clone_node(Some(m.keys[0])).unwrap(),
                                None,
                                None,
                                a,
                            ),
                            tracker.new_property_assignment(
                                None,
                                tracker.deep_clone_node(Some(m.keys[1])).unwrap(),
                                None,
                                None,
                                b,
                            ),
                        ]),
                        false,
                    )
                } else {
                    tracker.new_array_literal_expression(tracker.new_node_list(vec![a, b]), false)
                };
                let call = tracker.new_call_expression(
                    all_reference.expression(tracker, &all),
                    None,
                    None,
                    tracker.new_node_list(vec![argument]),
                    NodeFlags::None,
                );
                // Effect.all replaces the outer flow up to the flatMap step;
                // later steps then apply to its result.
                tracker.replace_piping_flow_prefix(
                    Some(source_file),
                    Some(&m.flow),
                    m.index + 1,
                    Some(call),
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}

// go: internal/fixables/zip_via_nested_flat_map_to_all.go:72 zipOperandExpression d911e22d9132bb74
// zipOperandExpression rebuilds an Effect.all operand: a clone of the
// standalone expression, or the pipe call truncated before the step.
pub(crate) fn zip_operand_expression(
    tracker: &NodeFactory,
    operand: ZipViaNestedFlatMapToAllOperand,
) -> P<Node> {
    if let Some(node) = operand.node {
        return tracker.deep_clone_node(Some(node)).unwrap();
    }
    let parts = operand.parts();
    let mut arguments = Vec::with_capacity(parts.len() - 1);
    for &argument in &parts[1..] {
        arguments.push(tracker.deep_clone_node(Some(argument)).unwrap());
    }
    tracker.new_call_expression(
        tracker.deep_clone_node(Some(parts[0])).unwrap(),
        None,
        None,
        tracker.new_node_list(arguments),
        NodeFlags::None,
    )
}
