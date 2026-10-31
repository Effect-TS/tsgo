use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::USE_EFFECT_CATCH_NO_SUCH_ELEMENT_INSTEAD_OF_WRAPPING_SUCCESSES_IN_OPTION_SOME_AND_CATCHING_NO_SUCH_ELEMENT_ERROR_INTO_OPTION_NONE_EFFECT_MAP_SOME_CATCH_NO_SUCH_ELEMENT;

// go: internal/fixables/map_some_catch_no_such_element.go:12 MapSomeCatchNoSuchElementFix c0a0f9c1f72c0564
pub static MAP_SOME_CATCH_NO_SUCH_ELEMENT_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "mapSomeCatchNoSuchElement",
    description: "Replace with Effect.catchNoSuchElement",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["mapSomeCatchNoSuchElement_fix"],
    run: |ctx: &mut Context<'_, '_>| -> Vec<CodeAction> {
        let sf = ctx.source_file;
        for m in ets_rules::analyze_map_some_catch_no_such_element(&mut ctx.type_parser, Some(sf)) {
            if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
                continue;
            }
            let action = ctx.new_fix_action(FixAction {
                description: "Replace with Effect.catchNoSuchElement".to_string(),
                run: Box::new(move |t| {
                    let callee = t.new_property_access_path(
                        &ets_typeparser::find_effect_module_reference(Some(sf))
                            .append(&["catchNoSuchElement"]),
                    );
                    t.collapse_piping_flow_transformations(
                        Some(sf),
                        Some(&m.flow),
                        m.start,
                        m.end,
                        PipingFlowTransformationReplacement {
                            callee: Some(callee),
                            ..Default::default()
                        },
                    );
                }),
            });
            if let Some(action) = action {
                return vec![action];
            }
        }
        Vec::new()
    },
});
