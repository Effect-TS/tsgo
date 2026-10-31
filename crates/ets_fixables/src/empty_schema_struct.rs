use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::NodeFlags;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::scanner;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::SCHEMA_STRUCT_AS_TOOL_PARAMETERS_BREAKS_STRUCTURED_OUTPUT_SCHEMA_GENERATION_FOR_SOME_PROVIDERS_USE_TOOL_EMPTY_PARAMS_INSTEAD_EFFECT_EMPTY_SCHEMA_STRUCT;

// go: internal/fixables/empty_schema_struct.go:13 EmptySchemaStructFix 143580e66ef21bb3
pub static EMPTY_SCHEMA_STRUCT_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "emptySchemaStruct",
    description: "Replace with Tool.EmptyParams",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["emptySchemaStruct_fix"],
    run: run_empty_schema_struct_fix,
});

// go: internal/fixables/empty_schema_struct.go:23 runEmptySchemaStructFix 79362792c14012ad
fn run_empty_schema_struct_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_empty_schema_struct(&mut ctx.type_parser, sf) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        // Only the tool parameters case has a replacement; it goes through the
        // Tool module receiver of the constructor, which resolves where the edit lands.
        let (true, Some(tool_namespace)) = (m.in_tool_parameters, m.tool_namespace) else {
            continue;
        };
        let call = m.call;

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with ".to_string()
                + &scanner::get_text_of_node(tool_namespace)
                + ".EmptyParams",
            run: Box::new(move |tracker| {
                let empty_params = tracker.new_property_access_expression(
                    tracker.deep_clone_node(Some(tool_namespace)).unwrap(),
                    None,
                    tracker.new_identifier("EmptyParams"),
                    NodeFlags::None,
                );
                tracker.replace_node(sf, Some(call), Some(empty_params), None);
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
