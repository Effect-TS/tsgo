use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_rewriter::Tracker;
use ets_shim::ast::{self, Kind, NodeFlags};
use ets_shim::checker::TypeFlags;
use ets_shim::core::alloc_str;
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;
use ets_typeparser::EffectMajorVersion;

use crate::*;

// go: internal/refactors/make_schema_opaque_with_ns.go:12 MakeSchemaOpaqueWithNs
pub static MAKE_SCHEMA_OPAQUE_WITH_NS: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "makeSchemaOpaqueWithNs",
    description: "Make Schema opaque with namespace",
    kind: "rewrite.effect.makeSchemaOpaqueWithNs",
    run: run_make_schema_opaque_with_ns,
});

// go: internal/refactors/make_schema_opaque_with_ns.go:19 runMakeSchemaOpaqueWithNs
pub(crate) fn run_make_schema_opaque_with_ns(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let Some(info) = find_schema_variable_declaration(ctx) else {
        return Vec::new();
    };

    let version = ctx.type_parser.supported_effect_version();
    let is_v4 = version == EffectMajorVersion::V4;

    let sf = ctx.source_file;
    let action = ctx.new_refactor_action(RefactorAction {
        description: "Make Schema opaque with namespace".to_string(),
        run: Box::new(move |tracker: &mut Tracker| {
            let schema_id = ets_typeparser::find_module_identifier(sf, "Schema");
            let orig_name = info.identifier.as_identifier().text();
            let new_name = orig_name.to_string() + "_";

            let is_e_object = info.schema_types.e().flags().intersects(TypeFlags::Object);

            // 1. Rename the original variable identifier (append "_")
            let new_identifier = tracker.new_identifier(alloc_str(&new_name));
            tracker.replace_node(sf, Some(info.identifier), Some(new_identifier), None);

            let mut insert_nodes = Vec::new();

            // 2. Opaque Type: export interface <Name> extends Schema.Schema.Type<typeof Name_>
            // Always uses Schema.Schema.Type, always an interface
            let opaque_type = create_opaque_interface(
                tracker, orig_name, &schema_id, "Schema", "Type", &new_name,
            );
            insert_nodes.push(opaque_type);

            // 3. Build namespace members
            let mut ns_members = Vec::new();

            // Encoded: export interface/type Encoded extends Schema.{Codec|Schema}.Encoded<typeof Name_>
            let mut encoded_ns = "Schema";
            if is_v4 {
                encoded_ns = "Codec";
            }
            let encoded_member = if is_e_object {
                create_opaque_interface(
                    tracker, "Encoded", &schema_id, encoded_ns, "Encoded", &new_name,
                )
            } else {
                create_opaque_type_alias(
                    tracker, "Encoded", &schema_id, encoded_ns, "Encoded", &new_name,
                )
            };
            ns_members.push(encoded_member);

            // Context/DecodingServices
            let context_member_name;
            if is_v4 {
                context_member_name = "DecodingServices";
                let context_member = create_opaque_type_alias(
                    tracker,
                    "DecodingServices",
                    &schema_id,
                    "Codec",
                    "DecodingServices",
                    &new_name,
                );
                ns_members.push(context_member);
            } else {
                context_member_name = "Context";
                let context_member = create_opaque_type_alias(
                    tracker, "Context", &schema_id, "Schema", "Context", &new_name,
                );
                ns_members.push(context_member);
            }

            // V4 only: EncodingServices
            if is_v4 {
                let es_member = create_opaque_type_alias(
                    tracker,
                    "EncodingServices",
                    &schema_id,
                    "Codec",
                    "EncodingServices",
                    &new_name,
                );
                ns_members.push(es_member);
            }

            // 4. Create namespace declaration
            let ns_block = tracker.new_module_block(tracker.new_node_list(ns_members));
            let ns_decl = tracker.new_module_declaration(
                Some(opaque_export_modifiers(tracker)),
                Kind::NamespaceKeyword,
                tracker.new_identifier(orig_name),
                Some(ns_block),
            );
            ast::set_parent_in_children(ns_decl);
            insert_nodes.push(ns_decl);

            // 5. Re-export const: export const <Name>: Schema.Schema<Name, Name.Encoded, Name.{Context|DecodingServices}> = Name_
            // Always uses Schema.Schema with 3 type args for the namespace variant
            let type_args = vec![
                tracker.new_type_reference_node(tracker.new_identifier(orig_name), None),
                tracker.new_type_reference_node(
                    tracker.new_qualified_name(
                        tracker.new_identifier(orig_name),
                        tracker.new_identifier("Encoded"),
                    ),
                    None,
                ),
                tracker.new_type_reference_node(
                    tracker.new_qualified_name(
                        tracker.new_identifier(orig_name),
                        tracker.new_identifier(context_member_name),
                    ),
                    None,
                ),
            ];

            let const_type = tracker.new_type_reference_node(
                tracker.new_qualified_name(
                    tracker.new_identifier(alloc_str(&schema_id)),
                    tracker.new_identifier("Schema"),
                ),
                Some(tracker.new_node_list(type_args)),
            );

            let const_decl = tracker.new_variable_declaration(
                tracker.new_identifier(orig_name),
                None,             // exclamationToken
                Some(const_type), // type annotation
                Some(tracker.new_identifier(alloc_str(&new_name))),
            );
            let const_decl_list = tracker.new_variable_declaration_list(
                tracker.new_node_list(vec![const_decl]),
                NodeFlags::Const,
            );
            let const_statement = tracker
                .new_variable_statement(Some(opaque_export_modifiers(tracker)), const_decl_list);
            ast::set_parent_in_children(const_statement);
            insert_nodes.push(const_statement);

            // Insert all nodes after the variable statement
            tracker.insert_nodes_after(sf, info.var_statement, insert_nodes);
        }),
    });
    let Some(mut action) = action else {
        return Vec::new();
    };
    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.makeSchemaOpaqueWithNs");
    vec![action]
}
