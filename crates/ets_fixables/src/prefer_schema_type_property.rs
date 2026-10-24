use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::Do_not_use_Schema_Schema_Type_typeof_X_to_extract_a_schema_s_type_Use_typeof_X_Type_instead_effect_preferSchemaTypeProperty;

// go: internal/fixables/prefer_schema_type_property.go:11 PreferSchemaTypePropertyFix
pub static PREFER_SCHEMA_TYPE_PROPERTY_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "preferSchemaTypeProperty",
    description: "Replace with typeof X.Type",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["preferSchemaTypeProperty_fix"],
    run: run_prefer_schema_type_property_fix,
});

// go: internal/fixables/prefer_schema_type_property.go:19 runPreferSchemaTypePropertyFix
fn run_prefer_schema_type_property_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    let matches = ets_rules::analyze_prefer_schema_type_property(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        let schema_name = m.schema_name;
        let type_reference = m.type_reference;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with 'typeof ".to_string() + &m.schema_name_text + ".Type'",
            run: Box::new(move |tracker| {
                let schema_type = tracker.new_qualified_name(
                    tracker.deep_clone_node(Some(schema_name)).unwrap(),
                    tracker.new_identifier("Type"),
                );
                let replacement = tracker.new_type_query_node(schema_type, None);
                tracker.replace_node(sf, Some(type_reference), Some(replacement), None);
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
