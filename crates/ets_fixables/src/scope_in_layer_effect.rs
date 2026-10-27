use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_LAYER_CONSTRUCTION_LEAVES_SCOPE_IN_THE_REQUIREMENT_SET_THE_SCOPED_API_REMOVES_SCOPE_FROM_THE_RESULTING_REQUIREMENTS_EFFECT_SCOPE_IN_LAYER_EFFECT;

// go: internal/fixables/scope_in_layer_effect.go:12 ScopeInLayerEffectScopedFix 4b5d20dbbc6f1fb1
pub static SCOPE_IN_LAYER_EFFECT_SCOPED_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "scopeInLayerEffectScoped",
    description: "Use scoped for Layer creation",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["scopeInLayerEffect_scoped"],
    run: run_scope_in_layer_effect_scoped_fix,
});

// go: internal/fixables/scope_in_layer_effect.go:20 runScopeInLayerEffectScopedFix 894c8bed0389a3f8
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

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Use scoped for Layer creation".to_string(),
            run: Box::new(move |tracker| {
                let scoped = tracker.new_property_access_path(
                    &ets_typeparser::find_module_reference(sf, "Layer").append(&["scoped"]),
                );
                tracker.replace_node(sf, Some(callee), Some(scoped), None);
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
