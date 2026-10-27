use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message =
    &tsdiag::THIS_TYPEOF_TYPE_QUERY_CAN_BE_REPLACED_WITH_0_EFFECT_UNNECESSARY_TYPEOF_TYPE;

// go: internal/fixables/unnecessary_typeof_type.go:11 UnnecessaryTypeofTypeFix a944919a9f56360e
pub static UNNECESSARY_TYPEOF_TYPE_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "unnecessaryTypeofType",
    description: "Replace with the matching named type",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["unnecessaryTypeofType_fix"],
    run: run_unnecessary_typeof_type_fix,
});

// go: internal/fixables/unnecessary_typeof_type.go:19 runUnnecessaryTypeofTypeFix c130e4c43fcfb8d2
fn run_unnecessary_typeof_type_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    let matches = ets_rules::analyze_unnecessary_typeof_type(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        let description = "Replace with '".to_string() + &m.replacement_text + "'";
        let query_node = m.query_node;
        let inner_entity_name = m.inner_entity_name;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                let replacement = tracker.deep_clone_node(Some(inner_entity_name));
                tracker.replace_node(sf, Some(query_node), replacement, None);
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
