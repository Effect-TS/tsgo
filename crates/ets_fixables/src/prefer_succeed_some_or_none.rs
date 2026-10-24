use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::NodeFlags;
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message =
    &tsdiag::Effect_0_expresses_this_Option_success_value_directly_effect_preferSucceedSomeOrNone;

// go: internal/fixables/prefer_succeed_some_or_none.go:13 PreferSucceedSomeOrNoneFix
pub static PREFER_SUCCEED_SOME_OR_NONE_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "preferSucceedSomeOrNone",
    description: "Replace with the direct Effect Option constructor",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["preferSucceedSomeOrNone_fix"],
    run: run_prefer_succeed_some_or_none_fix,
});

// go: internal/fixables/prefer_succeed_some_or_none.go:23 runPreferSucceedSomeOrNoneFix
fn run_prefer_succeed_some_or_none_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_prefer_succeed_some_or_none(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if m.flow.is_none()
            || m.transformation_count <= 0
            || m.replacement_name == "succeedSome" && m.value_node.is_none()
        {
            continue;
        }

        let description = "Replace with Effect.".to_string() + m.replacement_name;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                let module_node = match m.effect_module_node {
                    None => tracker.new_identifier(alloc_str(
                        &ets_typeparser::find_effect_module_identifier(sf),
                    )),
                    Some(module_node) => tracker.deep_clone_node(Some(module_node)).unwrap(),
                };
                let callee = tracker.new_property_access_expression(
                    module_node,
                    None,
                    tracker.new_identifier(m.replacement_name),
                    NodeFlags::None,
                );
                let mut replacement = callee;
                if let Some(value_node) = m.value_node {
                    let mut type_arguments = None;
                    if let Some(value_type_arguments) = m.value_type_arguments
                        && !value_type_arguments.nodes().is_empty()
                    {
                        let arguments = value_type_arguments
                            .nodes()
                            .iter()
                            .map(|&argument| tracker.deep_clone_node(Some(argument)).unwrap())
                            .collect();
                        type_arguments = Some(tracker.new_node_list(arguments));
                    }
                    replacement = tracker.new_call_expression(
                        callee,
                        None,
                        type_arguments,
                        tracker.new_node_list(vec![
                            tracker.deep_clone_node(Some(value_node)).unwrap(),
                        ]),
                        NodeFlags::None,
                    );
                }
                tracker.replace_piping_flow_prefix(
                    Some(sf),
                    m.flow.as_ref(),
                    m.transformation_count,
                    Some(replacement),
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
