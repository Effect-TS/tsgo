use ets_rewriter::NodeFactoryPaths;
use ets_typeparser::ModuleReference;
use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_rewriter::Tracker;
use ets_shim::ast::{self, Kind, ModifierList, Node, NodeFactory, NodeFlags};
use ets_shim::astnav;
use ets_shim::checker::TypeFlags;
use ets_shim::core::{P, alloc_str};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;
use ets_typeparser::{EffectMajorVersion, SchemaTypes};

use crate::*;

// go: internal/refactors/make_schema_opaque.go:13 MakeSchemaOpaque
pub static MAKE_SCHEMA_OPAQUE: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "makeSchemaOpaque",
    description: "Make Schema opaque",
    kind: "rewrite.effect.makeSchemaOpaque",
    run: run_make_schema_opaque,
});

// go: internal/refactors/make_schema_opaque.go:21 schemaVarInfo
// schemaVarInfo holds the result of finding a schema variable declaration.
pub(crate) struct SchemaVarInfo {
    pub(crate) identifier: P<Node>,    // the variable name identifier
    pub(crate) var_statement: P<Node>, // the enclosing VariableStatement
    #[expect(dead_code, reason = "Go fills the field and never reads it")]
    pub(crate) var_decl_list: P<Node>, // the VariableDeclarationList
    #[expect(dead_code, reason = "Go fills the field and never reads it")]
    pub(crate) initializer: P<Node>, // the initializer expression
    pub(crate) schema_types: SchemaTypes,
}

// go: internal/refactors/make_schema_opaque.go:31 findSchemaVariableDeclaration
// findSchemaVariableDeclaration walks the ancestor chain from the cursor to find a
// VariableDeclaration whose initializer has a Schema type. Returns nil if not found.
//
// Rust: Go's unused checker parameter drops (the checker is the context's type parser's); tsrs's
// `get_token_at_position` never returns nil.
pub(crate) fn find_schema_variable_declaration(ctx: &mut Context<'_, '_>) -> Option<SchemaVarInfo> {
    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    let mut node = Some(token);
    while let Some(n) = node.filter(|n| n.kind() != Kind::SourceFile) {
        node = n.parent();
        if n.kind() != Kind::VariableDeclaration {
            continue;
        }

        let var_decl = n.as_variable_declaration();

        // Must have an identifier name
        let name_node = var_decl.name();
        if name_node.kind() != Kind::Identifier {
            continue;
        }

        // Name must be within the selection span
        let name_pos = astnav::get_start_of_node(name_node, ctx.source_file, false);
        let name_end = name_node.end().expect_source("a parsed name");
        if span_pos(ctx) < name_pos || span_pos(ctx) >= name_end {
            continue;
        }

        // Must have an initializer
        let Some(initializer) = var_decl.initializer() else {
            continue;
        };

        // Parent chain: VariableDeclaration → VariableDeclarationList → VariableStatement
        let Some(var_decl_list) = n.parent().filter(|p| p.kind() == Kind::VariableDeclarationList)
        else {
            continue;
        };
        let Some(var_statement) =
            var_decl_list.parent().filter(|p| p.kind() == Kind::VariableStatement)
        else {
            continue;
        };

        // Check the initializer's type is a Schema type
        let Some(init_type) = ctx.type_parser.get_type_at_location(initializer) else {
            continue;
        };

        let Some(schema_types) = ctx.type_parser.effect_schema_types(init_type) else {
            continue;
        };

        return Some(SchemaVarInfo {
            identifier: name_node,
            var_statement,
            var_decl_list,
            initializer,
            schema_types,
        });
    }

    None
}

// go: internal/refactors/make_schema_opaque.go:96 createTypeofReference
// createTypeofReference creates: typeof <name>
pub(crate) fn create_typeof_reference(tracker: &NodeFactory, name: &str) -> P<Node> {
    tracker.new_type_query_node(tracker.new_identifier(alloc_str(name)), None)
}

// go: internal/refactors/make_schema_opaque.go:105 createSchemaPropertyAccess
// createSchemaPropertyAccess creates: <schema>.<ns>.<member> as a property access expression.
// Used in ExpressionWithTypeArguments for heritage clauses.
pub(crate) fn create_schema_property_access(
    tracker: &NodeFactory,
    schema: &ModuleReference,
    ns: &str,
    member: &str,
) -> P<Node> {
    tracker.new_property_access_path(&schema.append(&[ns, member]))
}

// go: internal/refactors/make_schema_opaque.go:111 createSchemaQualifiedName
// createSchemaQualifiedName creates: <schema>.<ns>.<member> as a qualified name.
// Used in TypeReferenceNode for type alias declarations.
pub(crate) fn create_schema_qualified_name(
    tracker: &NodeFactory,
    schema: &ModuleReference,
    ns: &str,
    member: &str,
) -> P<Node> {
    tracker.new_qualified_name_path(&schema.append(&[ns, member]))
}

// go: internal/refactors/make_schema_opaque.go:117 createHeritageTypeRef
// createHeritageTypeRef creates: <schema>.<ns>.<member><typeof <inferFrom>>
// as an ExpressionWithTypeArguments node for use in interface heritage clauses.
pub(crate) fn create_heritage_type_ref(
    tracker: &NodeFactory,
    schema: &ModuleReference,
    ns: &str,
    member: &str,
    infer_from: &str,
) -> P<Node> {
    tracker.new_expression_with_type_arguments(
        create_schema_property_access(tracker, schema, ns, member),
        Some(tracker.new_node_list(vec![create_typeof_reference(tracker, infer_from)])),
    )
}

// go: internal/refactors/make_schema_opaque.go:128 createTypeAliasTypeRef
// createTypeAliasTypeRef creates: <schema>.<ns>.<member><typeof <inferFrom>>
// as a TypeReferenceNode for use in type alias declarations.
pub(crate) fn create_type_alias_type_ref(
    tracker: &NodeFactory,
    schema: &ModuleReference,
    ns: &str,
    member: &str,
    infer_from: &str,
) -> P<Node> {
    tracker.new_type_reference_node(
        create_schema_qualified_name(tracker, schema, ns, member),
        Some(tracker.new_node_list(vec![create_typeof_reference(tracker, infer_from)])),
    )
}

// go: internal/refactors/make_schema_opaque.go:137 opaqueExportModifiers
pub(crate) fn opaque_export_modifiers(tracker: &NodeFactory) -> P<ModifierList> {
    tracker.new_modifier_list(vec![tracker.new_modifier(Kind::ExportKeyword)])
}

// go: internal/refactors/make_schema_opaque.go:144 createOpaqueInterface
// createOpaqueInterface creates: export interface <name> extends <heritage> { }
pub(crate) fn create_opaque_interface(
    tracker: &NodeFactory,
    name: &str,
    schema: &ModuleReference,
    ns: &str,
    member: &str,
    infer_from: &str,
) -> P<Node> {
    let heritage = create_heritage_type_ref(tracker, schema, ns, member, infer_from);
    let heritage_clause =
        tracker.new_heritage_clause(Kind::ExtendsKeyword, tracker.new_node_list(vec![heritage]));
    let node = tracker.new_interface_declaration(
        Some(opaque_export_modifiers(tracker)),
        tracker.new_identifier(alloc_str(name)),
        None, // typeParameters
        Some(tracker.new_node_list(vec![heritage_clause])),
        tracker.new_node_list(vec![]), // empty members
    );
    ast::set_parent_in_children(node);
    node
}

// go: internal/refactors/make_schema_opaque.go:163 createOpaqueTypeAlias
// createOpaqueTypeAlias creates: export type <name> = <schema>.<ns>.<member><typeof inferFrom>
pub(crate) fn create_opaque_type_alias(
    tracker: &NodeFactory,
    name: &str,
    schema: &ModuleReference,
    ns: &str,
    member: &str,
    infer_from: &str,
) -> P<Node> {
    let type_ref = create_type_alias_type_ref(tracker, schema, ns, member, infer_from);
    let node = tracker.new_type_alias_declaration(
        Some(opaque_export_modifiers(tracker)),
        tracker.new_identifier(alloc_str(name)),
        None, // typeParameters
        Some(type_ref),
    );
    ast::set_parent_in_children(node);
    node
}

// go: internal/refactors/make_schema_opaque.go:176 createOpaqueTypeDecl
// createOpaqueTypeDecl creates an interface (if isObject) or type alias for the given schema property.
pub(crate) fn create_opaque_type_decl(
    tracker: &NodeFactory,
    name: &str,
    schema: &ModuleReference,
    ns: &str,
    member: &str,
    infer_from: &str,
    is_object: bool,
) -> P<Node> {
    if is_object {
        return create_opaque_interface(tracker, name, schema, ns, member, infer_from);
    }
    create_opaque_type_alias(tracker, name, schema, ns, member, infer_from)
}

// go: internal/refactors/make_schema_opaque.go:183 runMakeSchemaOpaque
pub(crate) fn run_make_schema_opaque(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let Some(info) = find_schema_variable_declaration(ctx) else {
        return Vec::new();
    };

    let version = ctx.type_parser.supported_effect_version();
    let is_v4 = version == EffectMajorVersion::V4;

    let sf = ctx.source_file;
    let action = ctx.new_refactor_action(RefactorAction {
        description: "Make Schema opaque".to_string(),
        run: Box::new(move |tracker: &mut Tracker| {
            let schema = ets_typeparser::find_module_reference(sf, "Schema");
            let orig_name = info.identifier.as_identifier().text();
            let new_name = orig_name.to_string() + "_";

            let is_a_object = info.schema_types.a().flags().intersects(TypeFlags::Object);
            let is_e_object = info.schema_types.e().flags().intersects(TypeFlags::Object);

            // 1. Rename the original variable identifier (append "_")
            let new_identifier = tracker.new_identifier(alloc_str(&new_name));
            tracker.replace_node(sf, Some(info.identifier), Some(new_identifier), None);

            // Build all nodes to insert after the variable statement
            let mut insert_nodes = Vec::new();

            // 2. Opaque Type: export interface/type <Name> extends Schema.Schema.Type<typeof Name_>
            let opaque_type = create_opaque_type_decl(
                tracker,
                orig_name,
                &schema,
                "Schema",
                "Type",
                &new_name,
                is_a_object,
            );
            insert_nodes.push(opaque_type);

            // 3. Encoded: export interface/type <Name>Encoded extends Schema.{Codec|Schema}.Encoded<typeof Name_>
            let mut encoded_ns = "Schema";
            if is_v4 {
                encoded_ns = "Codec";
            }
            let encoded_name = orig_name.to_string() + "Encoded";
            let encoded_type = create_opaque_type_decl(
                tracker,
                &encoded_name,
                &schema,
                encoded_ns,
                "Encoded",
                &new_name,
                is_e_object,
            );
            insert_nodes.push(encoded_type);

            // 4. Context / DecodingServices
            let mut context_ns = "Schema";
            let mut context_member = "Context";
            let mut context_name = orig_name.to_string() + "Context";
            if is_v4 {
                context_ns = "Codec";
                context_member = "DecodingServices";
                context_name = orig_name.to_string() + "DecodingServices";
            }
            let context_type = create_opaque_type_alias(
                tracker,
                &context_name,
                &schema,
                context_ns,
                context_member,
                &new_name,
            );
            insert_nodes.push(context_type);

            // 5. V4 only: EncodingServices
            let mut encoding_services_name = String::new();
            if is_v4 {
                encoding_services_name = orig_name.to_string() + "EncodingServices";
                let es_type = create_opaque_type_alias(
                    tracker,
                    &encoding_services_name,
                    &schema,
                    "Codec",
                    "EncodingServices",
                    &new_name,
                );
                insert_nodes.push(es_type);
            }

            // 6. Re-export const: export const <Name>: Schema.{Codec|Schema}<...> = Name_
            let mut schema_type_ns = "Schema";
            if is_v4 {
                schema_type_ns = "Codec";
            }
            let mut type_args = vec![
                tracker.new_type_reference_node(tracker.new_identifier(orig_name), None),
                tracker.new_type_reference_node(
                    tracker.new_identifier(alloc_str(&encoded_name)),
                    None,
                ),
                tracker.new_type_reference_node(
                    tracker.new_identifier(alloc_str(&context_name)),
                    None,
                ),
            ];
            if is_v4 {
                type_args.push(tracker.new_type_reference_node(
                    tracker.new_identifier(alloc_str(&encoding_services_name)),
                    None,
                ));
            }

            let const_type = tracker.new_type_reference_node(
                tracker.new_qualified_name_path(&schema.append(&[schema_type_ns])),
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
    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.makeSchemaOpaque");
    vec![action]
}
