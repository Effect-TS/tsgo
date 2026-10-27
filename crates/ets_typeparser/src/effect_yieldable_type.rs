use ets_shim::checker::{SignatureKind, Type};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/effect_yieldable_type.go:11 TypeParser.EffectYieldableType ee42924e453a226a
    // EffectYieldableType resolves both plain Effect types and yieldable wrappers
    // that implement the asEffect() protocol.
    // For v3: delegates directly to EffectType (v3 models yieldable through Effect subtyping).
    // For v4: tries EffectType first; if that fails, looks for an asEffect property,
    // checks if it's callable, and tries EffectType on the return type of each call signature.
    // Returns nil if the type is not an Effect and not yieldable.
    pub fn effect_yieldable_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Effect> {
        let t = t.into()?;
        cached(
            self,
            |l| &mut l.effect_yieldable_type,
            t,
            |tp| {
                let version = tp.detect_effect_version();

                // For v3, yieldable types are modeled through Effect subtyping,
                // so EffectType alone is sufficient.
                if version != EffectMajorVersion::V4 {
                    return tp.effect_type(t);
                }

                // v4: first try plain Effect type
                if let Some(result) = tp.effect_type(t) {
                    return Some(result);
                }

                // v4: look for asEffect() protocol
                let as_effect_type = tp.get_type_of_property_by_name(t, "asEffect")?;

                let signatures =
                    tp.checker.get_signatures_of_type_exported(as_effect_type, SignatureKind::Call);
                for &sig in signatures {
                    let return_type = tp.checker.get_return_type_of_signature_exported(sig);
                    if let Some(result) = tp.effect_type(return_type) {
                        return Some(result);
                    }
                }

                None
            },
        )
    }
}
