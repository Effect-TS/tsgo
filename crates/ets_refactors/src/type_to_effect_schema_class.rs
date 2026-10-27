use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

use crate::*;

// go: internal/refactors/type_to_effect_schema_class.go:10 TypeToEffectSchemaClass 7494e6a522ad8dcc
pub static TYPE_TO_EFFECT_SCHEMA_CLASS: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "typeToEffectSchemaClass",
    description: "Generate Schema.Class from type",
    kind: "rewrite.effect.typeToEffectSchemaClass",
    run: run_type_to_effect_schema_class,
});

// go: internal/refactors/type_to_effect_schema_class.go:17 runTypeToEffectSchemaClass 28a2b4568a346499
pub(crate) fn run_type_to_effect_schema_class(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let Some(matched_node) = find_interface_or_type_alias(ctx) else {
        return Vec::new();
    };

    // Schema.Class is not applicable when the type has index signatures
    if ets_schemagen::has_index_signatures(matched_node) {
        return Vec::new();
    }

    let version = ctx.type_parser.supported_effect_version();

    let sf = ctx.source_file;
    let Some(mut action) = ctx.new_refactor_action(RefactorAction {
        description: "Generate Schema.Class from type".to_string(),
        run: Box::new(move |tracker| {
            let generator = ets_schemagen::new(tracker, sf, version);
            let new_node = generator.process(matched_node, true);
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
    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.typeToEffectSchemaClass");
    vec![action]
}
