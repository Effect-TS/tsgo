use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, NodeFlags};
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::This_code_uses_instanceof_with_an_Effect_Schema_type_Schema_is_is_the_schema_aware_runtime_check_for_this_case_effect_instanceOfSchema;

// go: internal/fixables/instance_of_schema.go:13 InstanceOfSchemaFix
pub static INSTANCE_OF_SCHEMA_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "instanceOfSchema",
    description: "Replace with Schema.is",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["instanceOfSchema_fix"],
    run: run_instance_of_schema_fix,
});

// go: internal/fixables/instance_of_schema.go:21 runInstanceOfSchemaFix
fn run_instance_of_schema_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_instance_of_schema(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Schema.is".to_string(),
            run: Box::new(move |tracker| {
                let cloned_left = tracker.deep_clone_node(Some(m.left_expr)).unwrap();
                let cloned_right = tracker.deep_clone_node(Some(m.right_expr)).unwrap();

                // Build Schema.is property access
                let schema_is_access = tracker.new_property_access_expression(
                    tracker.new_identifier(alloc_str(&ets_typeparser::find_module_identifier(
                        Some(sf),
                        "Schema",
                    ))),
                    None,
                    tracker.new_identifier("is"),
                    NodeFlags::None,
                );

                // Build inner call: Schema.is(right)
                let inner_call = tracker.new_call_expression(
                    schema_is_access,
                    None,
                    None,
                    tracker.new_node_list(vec![cloned_right]),
                    NodeFlags::None,
                );

                // Build outer call: Schema.is(right)(left)
                let outer_call = tracker.new_call_expression(
                    inner_call,
                    None,
                    None,
                    tracker.new_node_list(vec![cloned_left]),
                    NodeFlags::None,
                );

                ast::set_parent_in_children(outer_call);
                tracker.replace_node(sf, Some(m.instance_of_node), Some(outer_call), None);
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
