use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_shim::ast::{Kind, Node};
use ets_shim::astnav;
use ets_shim::core::P;
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

use crate::*;

// go: internal/refactors/type_to_effect_schema.go:12 TypeToEffectSchema
pub static TYPE_TO_EFFECT_SCHEMA: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "typeToEffectSchema",
    description: "Generate Effect.Schema from type",
    kind: "rewrite.effect.typeToEffectSchema",
    run: run_type_to_effect_schema,
});

// go: internal/refactors/type_to_effect_schema.go:22 findInterfaceOrTypeAlias
// findInterfaceOrTypeAlias walks the ancestor chain from the token at the cursor
// to find an InterfaceDeclaration or TypeAliasDeclaration whose name overlaps the
// selection span, and that has no type parameters.
pub(crate) fn find_interface_or_type_alias(ctx: &Context<'_, '_>) -> Option<P<Node>> {
    let span_pos = span_pos(ctx);
    let token = astnav::get_token_at_position(ctx.source_file, span_pos);

    let ancestors = std::iter::successors(Some(token), |n| n.parent());
    for node in ancestors.take_while(|node| node.kind() != Kind::SourceFile) {
        match node.kind() {
            Kind::InterfaceDeclaration => {
                let iface = node.as_interface_declaration();
                // Must have no type parameters
                if iface.type_parameters().is_some_and(|tps| !tps.nodes().is_empty()) {
                    continue;
                }
                // Name must overlap the selection
                let name = iface.name();
                let name_pos = astnav::get_start_of_node(name, ctx.source_file, false);
                let name_end = name.end().expect_source("a parsed node");
                if span_pos < name_pos || span_pos >= name_end {
                    continue;
                }
                return Some(node);
            }
            Kind::TypeAliasDeclaration => {
                let ta = node.as_type_alias_declaration();
                // Must have no type parameters
                if ta.type_parameters().is_some_and(|tps| !tps.nodes().is_empty()) {
                    continue;
                }
                // Name must overlap the selection
                let name = ta.name();
                let name_pos = astnav::get_start_of_node(name, ctx.source_file, false);
                let name_end = name.end().expect_source("a parsed node");
                if span_pos < name_pos || span_pos >= name_end {
                    continue;
                }
                return Some(node);
            }
            _ => {}
        }
    }

    None
}

// go: internal/refactors/type_to_effect_schema.go:64 runTypeToEffectSchema
pub(crate) fn run_type_to_effect_schema(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let Some(matched_node) = find_interface_or_type_alias(ctx) else {
        return Vec::new();
    };

    let version = ctx.type_parser.supported_effect_version();

    let sf = ctx.source_file;
    let Some(mut action) = ctx.new_refactor_action(RefactorAction {
        description: "Generate Effect.Schema from type".to_string(),
        run: Box::new(move |tracker| {
            let generator = ets_schemagen::new(tracker, sf, version);
            let new_node = generator.process(matched_node, false);
            if let Some(new_node) = new_node {
                tracker.insert_node_before(
                    sf,
                    Some(matched_node),
                    Some(new_node),
                    true,
                    ets_rewriter::LEADING_TRIVIA_OPTION_NONE,
                );
            }
        }),
    }) else {
        return Vec::new();
    };
    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.typeToEffectSchema");
    vec![action]
}
