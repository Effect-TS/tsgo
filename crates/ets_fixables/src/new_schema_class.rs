use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, NodeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::This_Schema_class_is_constructed_with_new_0_make_can_be_used_to_construct_the_Schema_class_instance_effect_newSchemaClass;

// go: internal/fixables/new_schema_class.go:12 NewSchemaClassFix
pub static NEW_SCHEMA_CLASS_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "newSchemaClass",
    description: "Replace new Schema class construction with .make",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["newSchemaClass_fix"],
    run: run_new_schema_class_fix,
});

// go: internal/fixables/new_schema_class.go:20 runNewSchemaClassFix
fn run_new_schema_class_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    let matches = ets_rules::analyze_new_schema_class(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with .make".to_string(),
            run: Box::new(move |tracker| {
                let cloned_class_expr = tracker.deep_clone_node(Some(m.class_expr)).unwrap();
                let make_access = tracker.new_property_access_expression(
                    cloned_class_expr,
                    None,
                    tracker.new_identifier("make"),
                    NodeFlags::None,
                );

                let mut cloned_args = Vec::new();
                if !m.arguments.is_empty() {
                    cloned_args = m
                        .arguments
                        .iter()
                        .map(|&arg| tracker.deep_clone_node(Some(arg)).unwrap())
                        .collect();
                }

                let replacement = tracker.new_call_expression(
                    make_access,
                    None,
                    None,
                    tracker.new_node_list(cloned_args),
                    NodeFlags::None,
                );

                ast::set_parent_in_children(replacement);
                tracker.replace_node(sf, Some(m.new_node), Some(replacement), None);
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
