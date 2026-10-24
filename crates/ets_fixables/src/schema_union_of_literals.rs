use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, NodeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::This_Schema_Union_contains_multiple_Schema_Literal_members_and_can_be_simplified_to_a_single_Schema_Literal_call_effect_schemaUnionOfLiterals;

// go: internal/fixables/schema_union_of_literals.go:12 SchemaUnionOfLiteralsFix
pub static SCHEMA_UNION_OF_LITERALS_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "schemaUnionOfLiterals",
    description: "Replace with a single Schema.Literal call",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["schemaUnionOfLiterals_fix"],
    run: run_schema_union_of_literals_fix,
});

// go: internal/fixables/schema_union_of_literals.go:20 runSchemaUnionOfLiteralsFix
//
// Rust: Go's `NewCallExpression` would take the nil clone of a nil first literal expression; tsrs's needs a node,
// and the rule sets it for every match (`expect`).
fn run_schema_union_of_literals_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_schema_union_of_literals(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        // Capture loop variables for the closure
        let union_call_node = m.union_call_node;
        let first_literal_expression = m.first_literal_expression;
        let all_literal_args = m.all_literal_args;

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with a single Schema.Literal call".to_string(),
            run: Box::new(move |tracker| {
                let cloned_expression = tracker
                    .deep_clone_node(first_literal_expression)
                    .expect("the first Schema.Literal callee is set");
                let cloned_args: Vec<_> = all_literal_args
                    .iter()
                    .map(|&arg| tracker.deep_clone_node(Some(arg)).unwrap())
                    .collect();
                let new_call = tracker.new_call_expression(
                    cloned_expression,
                    None,
                    None,
                    tracker.new_node_list(cloned_args),
                    NodeFlags::None,
                );
                ast::set_parent_in_children(new_call);
                tracker.replace_node(sf, Some(union_call_node), Some(new_call), None);
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
