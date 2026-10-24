use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast;
use ets_shim::core::alloc_str;
use ets_shim::diagnostics as tsdiag;
use ets_shim::ls::CodeAction;

// go: internal/fixables/class_self_mismatch.go:12 ClassSelfMismatchFix
pub static CLASS_SELF_MISMATCH_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "classSelfMismatch",
    description: "Replace Self type parameter with the correct class name",
    error_codes: vec![
        tsdiag::The_Self_type_parameter_for_this_class_should_be_0_effect_classSelfMismatch.code(),
    ],
    fix_ids: vec!["classSelfMismatch_fix"],
    run: run_class_self_mismatch_fix,
});

// go: internal/fixables/class_self_mismatch.go:20 runClassSelfMismatchFix
fn run_class_self_mismatch_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_class_self_mismatch(&mut ctx.type_parser, sf);
    for m in matches {
        let diag_range = m.location;
        if !diag_range.intersects(ctx.span) && !ctx.span.contained_by(diag_range) {
            continue;
        }

        // Determine the target node to replace. If the Self type node is a TypeReferenceNode,
        // replace only the TypeName identifier (preserving any type arguments).
        // Otherwise, replace the entire Self type node.
        let mut target_node = m.self_type_node;
        if ast::is_type_reference_node(m.self_type_node) {
            target_node = m.self_type_node.as_type_reference_node().type_name();
        }

        let description = format!("Replace '{}' with '{}'", m.actual_name, m.expected_name);
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                let replacement = tracker.new_identifier(alloc_str(&m.expected_name));
                tracker.replace_node(sf, Some(target_node), Some(replacement), None);
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
