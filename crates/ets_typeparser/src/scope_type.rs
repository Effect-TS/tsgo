use ets_shim::ast::SymbolFlags;
use ets_shim::checker::{SignatureKind, Type};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/scope_type.go:11 ScopeTypeId 9af8a8881e3c6385
// ScopeTypeId is the property key for Scope's variance struct.
pub const SCOPE_TYPE_ID: &str = "~effect/Scope";

impl TypeParser<'_> {
    // go: internal/typeparser/scope_type.go:17 TypeParser.ScopeType cde065da3b7d7bb0
    // ScopeType returns t itself when it is an Effect Scope type, or nil otherwise.
    // For v4, this checks for the "~effect/Scope" computed property.
    // For v3/unknown, this checks that the type is "pipeable" (has a callable pipe property)
    // and that any required non-optional property's symbol name contains "ScopeTypeId".
    pub fn scope_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<P<Type>> {
        let t = t.into()?;
        cached(
            self,
            |l| &mut l.scope_type,
            t,
            |tp| {
                let version = tp.detect_effect_version();
                if version == EffectMajorVersion::V4 {
                    tp.get_type_of_property_by_name(t, SCOPE_TYPE_ID)?;
                    return Some(t);
                }

                // v3 / unknown: check that the type is "pipeable"
                let pipe_type = tp.get_type_of_property_by_name(t, "pipe")?;
                let signatures =
                    tp.checker.get_signatures_of_type_exported(pipe_type, SignatureKind::Call);
                if signatures.is_empty() {
                    return None;
                }

                // Check if any required non-optional property's symbol name contains "ScopeTypeId"
                for &prop in tp.checker.get_properties_of_type_exported(t) {
                    if !prop.flags().intersects(SymbolFlags::Property) {
                        continue;
                    }
                    if prop.flags().intersects(SymbolFlags::Optional) {
                        continue;
                    }
                    if prop.value_declaration().is_none() {
                        continue;
                    }
                    if prop.name().contains("ScopeTypeId") {
                        return Some(t);
                    }
                }

                None
            },
        )
    }
}
