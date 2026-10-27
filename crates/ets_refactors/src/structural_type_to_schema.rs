use std::sync::LazyLock;

use ets_refactor::{Context, Refactor};
use ets_shim::ast::{self, Kind, ModifierFlags};
use ets_shim::core::collections::OrderedMap;
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

use crate::*;

// go: internal/refactors/structural_type_to_schema.go:12 StructuralTypeToSchema 6ccef4fd59339888
pub static STRUCTURAL_TYPE_TO_SCHEMA: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "structuralTypeToSchema",
    description: "Refactor to Schema (Structural)",
    kind: "rewrite.effect.structuralTypeToSchema",
    run: run_structural_type_to_schema,
});

// go: internal/refactors/structural_type_to_schema.go:19 runStructuralTypeToSchema 3a43fb21e4ce4d93
// Rust: the edit closure uses the type parser, so the action is built with `new_refactor_action_with_type_parser`
// (Go's closure captures `ctx.TypeParser` and `c`).
pub(crate) fn run_structural_type_to_schema(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let Some(matched_node) = find_interface_or_type_alias(ctx) else {
        return Vec::new();
    };

    let version = ctx.type_parser.supported_effect_version();

    // Get the name node and resolve the type
    let name_node = match matched_node.kind() {
        Kind::InterfaceDeclaration => matched_node.as_interface_declaration().name(),
        _ => matched_node.as_type_alias_declaration().name(),
    };

    let Some(t) = ctx.type_parser.get_type_at_location(name_node) else {
        return Vec::new();
    };

    let type_name = name_node.as_identifier().text();
    let is_exported =
        ast::get_combined_modifier_flags(matched_node).intersects(ModifierFlags::Export);

    let sf = ctx.source_file;
    let Some(mut action) = ctx.new_refactor_action_with_type_parser(
        "Refactor to Schema (Recursive Structural)".to_string(),
        |tracker, tp| {
            let mut generator = ets_schemagen::new_structural_schema_gen(tracker, sf, version);
            let mut type_map = OrderedMap::default();
            type_map.insert(type_name.to_string(), t);
            let stmts = generator.process(tp, &type_map, matched_node, is_exported);
            for &stmt in stmts.iter().rev() {
                tracker.insert_node_before(
                    sf,
                    Some(matched_node),
                    Some(stmt),
                    true,
                    ets_rewriter::LEADING_TRIVIA_OPTION_NONE,
                );
            }
        },
    ) else {
        return Vec::new();
    };
    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.structuralTypeToSchema");
    vec![action]
}
