use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message =
    &tsdiag::DO_NOT_USE_0_TYPEOF_X_USE_TYPEOF_X_1_INSTEAD_EFFECT_PREFER_SCHEMA_TYPE_PROPERTY;

// go: internal/fixables/prefer_schema_type_property.go:11 PreferSchemaTypePropertyFix
pub static PREFER_SCHEMA_TYPE_PROPERTY_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "preferSchemaTypeProperty",
    description: "Replace with typeof X.Type or typeof X.Encoded",
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
        let property_name = m.property_name;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with 'typeof ".to_string()
                + &m.schema_name_text
                + "."
                + property_name
                + "'",
            run: Box::new(move |tracker| {
                let schema_type = tracker.new_qualified_name(
                    tracker.deep_clone_node(Some(schema_name)).unwrap(),
                    tracker.new_identifier(property_name),
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
