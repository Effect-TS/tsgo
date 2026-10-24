use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, NodeFlags, TokenFlags};
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::This_Schema_Struct_includes_a_tag_field_Schema_TaggedStruct_is_the_tagged_struct_form_for_this_pattern_and_makes_the_tag_optional_in_the_constructor_effect_schemaStructWithTag;

// go: internal/fixables/schema_struct_with_tag.go:13 SchemaStructWithTagFix
pub static SCHEMA_STRUCT_WITH_TAG_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "schemaStructWithTag",
    description: "Rewrite as Schema.TaggedStruct",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["schemaStructWithTag_fix"],
    run: run_schema_struct_with_tag_fix,
});

// go: internal/fixables/schema_struct_with_tag.go:21 runSchemaStructWithTagFix
fn run_schema_struct_with_tag_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_schema_struct_with_tag(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        // Capture loop variables for the closure
        let tag_value = m.tag_value;
        let other_properties = m.other_properties;
        let call_node = m.call_node;
        let schema_expr = m.schema_expr;

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Rewrite as Schema.TaggedStruct".to_string(),
            run: Box::new(move |tracker| {
                let schema_module = match schema_expr {
                    Some(schema_expr) => tracker.deep_clone_node(Some(schema_expr)).unwrap(),
                    None => tracker.new_identifier(alloc_str(
                        &ets_typeparser::find_module_identifier(sf, "Schema"),
                    )),
                };
                // Build Schema.TaggedStruct property access, preserving the receiver.
                let tagged_struct_access = tracker.new_property_access_expression(
                    schema_module,
                    None,
                    tracker.new_identifier("TaggedStruct"),
                    NodeFlags::None,
                );

                // Build the tag value string literal argument
                let tag_literal =
                    tracker.new_string_literal(alloc_str(&tag_value), TokenFlags::None);

                // Deep-clone each remaining property and build the new object literal
                let cloned_props: Vec<_> = other_properties
                    .iter()
                    .map(|&prop| tracker.deep_clone_node(Some(prop)).unwrap())
                    .collect();
                let new_obj_literal = tracker
                    .new_object_literal_expression(tracker.new_node_list(cloned_props), false);

                // Build Schema.TaggedStruct("tagValue", { ...rest })
                let new_call = tracker.new_call_expression(
                    tagged_struct_access,
                    None,
                    None,
                    tracker.new_node_list(vec![tag_literal, new_obj_literal]),
                    NodeFlags::None,
                );

                ast::set_parent_in_children(new_call);
                tracker.replace_node(sf, Some(call_node), Some(new_call), None);
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
