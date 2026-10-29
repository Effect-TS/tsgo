use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::NodeOptions;
use ets_rules::TypeUnionToSchemaUnionMatch;
use ets_shim::ast::{self, Kind, ModifierFlags, Node, NodeFlags, SourceFile, SymbolFlags};
use ets_shim::checker::Checker;
use ets_shim::core::{P, alloc_str};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::ls::format;
use ets_shim::scanner;
use ets_typeparser::{EffectMajorVersion, TypeParser};

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_TYPE_ALIAS_UNIONS_SCHEMA_0_TYPES_WITHOUT_A_RUNTIME_SCHEMA_COMPOSE_THEM_WITH_SCHEMA_UNION_AND_DERIVE_THE_TYPE_FROM_IT_EFFECT_TYPE_UNION_TO_SCHEMA_UNION;

// go: internal/fixables/type_union_to_schema_union.go:20 TypeUnionToSchemaUnionFix de5d7a45350ca285
pub static TYPE_UNION_TO_SCHEMA_UNION_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "typeUnionToSchemaUnion",
    description: "Convert to Schema.Union",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["typeUnionToSchemaUnion_fix"],
    run: run_type_union_to_schema_union_fix,
});

// go: internal/fixables/type_union_to_schema_union.go:28 runTypeUnionToSchemaUnionFix d1b9671cce6726db
fn run_type_union_to_schema_union_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    let matches = ets_rules::analyze_type_union_to_schema_union(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        // The const is inserted before the alias, so a member declared later in
        // the file would be used before its declaration (TS2448).
        if type_union_to_schema_union_has_late_member(&mut ctx.type_parser, sf, &m) {
            return Vec::new();
        }

        let alias = m.alias;
        let name = type_union_to_schema_union_free_name(ctx.checker(), alias);
        let is_v4 = ctx.type_parser.supported_effect_version() == EffectMajorVersion::V4;

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Convert to Schema.Union".to_string(),
            run: Box::new(move |tracker| {
                let schemas: Vec<P<Node>> = m
                    .members
                    .iter()
                    .map(|member| {
                        tracker.new_property_access_path(
                            &ets_rules::type_union_to_schema_union_name_parts(member.name),
                        )
                    })
                    .collect();
                let mut arguments = schemas.clone();
                if is_v4 {
                    arguments = vec![
                        tracker.new_array_literal_expression(tracker.new_node_list(schemas), false),
                    ];
                }
                let union = tracker.new_call_expression(
                    tracker.new_property_access_path(
                        &ets_typeparser::find_module_reference(Some(sf), "Schema")
                            .append(&["Union"]),
                    ),
                    None,
                    None,
                    tracker.new_node_list(arguments),
                    NodeFlags::None,
                );

                let mut modifiers = None;
                if ast::has_syntactic_modifier(alias, ModifierFlags::Export) {
                    modifiers = Some(
                        tracker.new_modifier_list(vec![tracker.new_modifier(Kind::ExportKeyword)]),
                    );
                }
                let declaration = tracker.new_variable_declaration(
                    tracker.new_identifier(alloc_str(&name)),
                    None,
                    None,
                    Some(union),
                );
                let statement = tracker.new_variable_statement(
                    modifiers,
                    tracker.new_variable_declaration_list(
                        tracker.new_node_list(vec![declaration]),
                        NodeFlags::Const,
                    ),
                );
                ast::set_parent_in_children(statement);

                let pos = scanner::get_token_pos_of_node(alias, sf, true);
                // Rust: tsrs's `NodeOptions` has crate-private fields, so the literal is a default with Go's field set.
                let mut options = NodeOptions::default();
                options.suffix = type_union_to_schema_union_insertion_suffix(sf, pos);
                tracker.insert_node_at(sf, pos, statement, options);

                let replacement = tracker.new_type_query_node(
                    tracker.new_qualified_name(
                        tracker.new_identifier(alloc_str(&name)),
                        tracker.new_identifier(m.side),
                    ),
                    None,
                );
                tracker.replace_node(
                    sf,
                    alias.as_type_alias_declaration().type_(),
                    Some(replacement),
                    None,
                );
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }
    Vec::new()
}

// go: internal/fixables/type_union_to_schema_union.go:96 typeUnionToSchemaUnionInsertionSuffix 488a6ef545546b4d
// typeUnionToSchemaUnionInsertionSuffix separates the inserted const from the
// alias at pos: when the alias starts its line, the const gets its own line with
// the alias indentation; when other code precedes the alias on its line (an
// inline block body, a previous statement, a comment), the const stays inline.
fn type_union_to_schema_union_insertion_suffix(sf: P<SourceFile>, pos: u32) -> String {
    let line_start = format::get_line_start_position_for_position(pos, sf);
    let line_prefix = &sf.text()[line_start as usize..pos as usize];
    if !line_prefix.trim_start_matches([' ', '\t']).is_empty() {
        return " ".to_string();
    }
    "\n".to_string() + line_prefix
}

// go: internal/fixables/type_union_to_schema_union.go:108 typeUnionToSchemaUnionHasLateMember 1749604452648681
// typeUnionToSchemaUnionHasLateMember reports whether any member, or any prefix
// of a qualified member such as Models in Models.Cat, is declared in this file
// after the alias. The local symbol is checked, so `const Late = Imported`
// declared after the alias counts as late.
fn type_union_to_schema_union_has_late_member(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    m: &TypeUnionToSchemaUnionMatch,
) -> bool {
    let alias_end = m.alias.end();
    for member in &m.members {
        let mut name = member.name;
        loop {
            if let Some(symbol) = tp.get_symbol_at_location(name) {
                for &declaration in symbol.declarations() {
                    if ast::get_source_file_of_node(declaration) == Some(sf)
                        && declaration.pos() >= alias_end
                    {
                        return true;
                    }
                }
            }
            if name.kind() != Kind::QualifiedName {
                break;
            }
            name = name.as_qualified_name().left();
        }
    }
    false
}

// go: internal/fixables/type_union_to_schema_union.go:130 typeUnionToSchemaUnionFreeName 47d592e8d0de2bc5
// typeUnionToSchemaUnionFreeName picks the const name: the alias name, or
// <Alias>Schema, <Alias>Schema2, ... when a value with that name is in scope.
fn type_union_to_schema_union_free_name(c: &mut Checker, alias: P<Node>) -> String {
    let base = alias.name().expect("a type alias name").text();
    let mut is_free = |candidate: &str| -> bool {
        c.resolve_name_exported(
            candidate,
            Some(alias),
            SymbolFlags::Value | SymbolFlags::Alias,
            false,
        )
        .is_none()
    };
    if is_free(base) {
        return base.to_string();
    }
    let candidate = base.to_string() + "Schema";
    if is_free(&candidate) {
        return candidate;
    }
    let mut i: i64 = 2;
    loop {
        let candidate = base.to_string() + "Schema" + &i.to_string();
        if is_free(&candidate) {
            return candidate;
        }
        i += 1;
    }
}
