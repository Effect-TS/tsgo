use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::NodeFlags;
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_LAYER_CONSTRUCTION_LEAVES_SCOPE_IN_THE_REQUIREMENT_SET_THE_SCOPED_API_REMOVES_SCOPE_FROM_THE_RESULTING_REQUIREMENTS_EFFECT_SCOPE_IN_LAYER_EFFECT;

// go: internal/fixables/scope_in_layer_effect.go:13 ScopeInLayerEffectScopedFix
pub static SCOPE_IN_LAYER_EFFECT_SCOPED_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "scopeInLayerEffectScoped",
    description: "Use scoped for Layer creation",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["scopeInLayerEffect_scoped"],
    run: run_scope_in_layer_effect_scoped_fix,
});

// go: internal/fixables/scope_in_layer_effect.go:21 runScopeInLayerEffectScopedFix
fn run_scope_in_layer_effect_scoped_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    let matches = ets_rules::analyze_scope_in_layer_effect(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        // Class declaration matches don't have a constructor callee to replace.
        let Some(callee) = m.callee else {
            continue;
        };

        let method_identifier = m.method_identifier;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Use scoped for Layer creation".to_string(),
            run: Box::new(move |tracker| {
                if let Some(method_identifier) = method_identifier {
                    let scoped = tracker.new_identifier("scoped");
                    tracker.replace_node(sf, Some(method_identifier), Some(scoped), None);
                } else {
                    let layer_module = tracker.new_identifier(alloc_str(
                        &ets_typeparser::find_module_identifier(sf, "Layer"),
                    ));
                    let scoped = tracker.new_property_access_expression(
                        layer_module,
                        None,
                        tracker.new_identifier("scoped"),
                        NodeFlags::None,
                    );
                    tracker.replace_node(sf, Some(callee), Some(scoped), None);
                }
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
