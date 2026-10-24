use ets_shim::ast::{Symbol, SymbolFlags};
use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/context_tag.go:13 TypeParser.ContextTag
    // ContextTag parses a v3 Context.Tag type and extracts Identifier, Shape parameters.
    // Returns nil if the type is not a v3 Context.Tag.
    pub fn context_tag(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Service> {
        let t = t.into()?;
        cached(
            self,
            |l| &mut l.context_tag,
            t,
            |tp| {
                if tp.detect_effect_version() != EffectMajorVersion::V3 {
                    return None;
                }
                if !tp.is_pipeable_type(t) {
                    return None;
                }

                let props = tp.checker.get_properties_of_type_exported(t);

                // Filter to required, non-optional properties with a value declaration
                let mut candidates: Vec<P<Symbol>> = Vec::new();
                for &prop in props {
                    if !prop.flags().intersects(SymbolFlags::Property) {
                        continue;
                    }
                    if prop.flags().intersects(SymbolFlags::Optional) {
                        continue;
                    }
                    if prop.value_declaration().is_none() {
                        continue;
                    }
                    candidates.push(prop);
                }

                if candidates.is_empty() {
                    return None;
                }

                ets_go::sort::slice_stable(&mut candidates, |i, j| {
                    let i_has = i.name().contains("TypeId");
                    let j_has = j.name().contains("TypeId");
                    i_has && !j_has
                });

                for prop in candidates {
                    let prop_type = tp.checker.get_type_of_symbol_at_location(prop, None);
                    if let Some(result) = tp.parse_service_variance_struct(prop_type) {
                        return Some(result);
                    }
                }

                None
            },
        )
    }

    // go: internal/typeparser/context_tag.go:70 TypeParser.IsContextTag
    // IsContextTag returns true if the type has the Context.Tag variance struct.
    pub fn is_context_tag(&mut self, t: impl Into<Option<P<Type>>>) -> bool {
        self.context_tag(t).is_some()
    }
}
