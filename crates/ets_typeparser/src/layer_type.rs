use ets_shim::ast::{Node, SourceFile, Symbol, SymbolFlags};
use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/layer_type.go:11 effectLayerModuleDescriptor 68a692cb72a411d0
pub(crate) static EFFECT_LAYER_MODULE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Layer,
        "effect",
        Some(&is_layer_type_source_file),
    );

// go: internal/typeparser/layer_type.go:14 LayerTypeId 58355dd578038478
// LayerTypeId is the property key for Layer's variance struct.
pub const LAYER_TYPE_ID: &str = "~effect/Layer";

// go: internal/typeparser/layer_type.go:17 Layer 18a890587ce0a99d
// Layer represents parsed Layer<ROut, E, RIn> type parameters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Layer {
    pub(crate) r_out: P<Type>, // Provided services (contravariant)
    pub(crate) e: P<Type>,     // Error type (covariant)
    pub(crate) r_in: P<Type>,  // Required services (covariant)
}

impl Layer {
    // go: internal/typeparser/layer_type.go:23 Layer.ROut c71a15db01afbb53
    pub fn r_out(&self) -> P<Type> {
        self.r_out
    }

    // go: internal/typeparser/layer_type.go:25 Layer.E ef30a4dbbb4ac9f5
    pub fn e(&self) -> P<Type> {
        self.e
    }

    // go: internal/typeparser/layer_type.go:27 Layer.RIn 82e5776df34c707a
    pub fn r_in(&self) -> P<Type> {
        self.r_in
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/layer_type.go:30 TypeParser.parseLayerVarianceStruct 05e69b0a2d490952
    // parseLayerVarianceStruct extracts ROut, E, RIn from a Layer variance struct type.
    pub(crate) fn parse_layer_variance_struct(&mut self, t: Option<P<Type>>) -> Option<Layer> {
        let r_out = self.extract_contravariant_type(t, "_ROut")?;

        let e = self.extract_covariant_type(t, "_E")?;

        let r_in = self.extract_covariant_type(t, "_RIn")?;

        Some(Layer { r_out, e, r_in })
    }

    // go: internal/typeparser/layer_type.go:53 TypeParser.LayerType 4f591014306719e6
    // LayerType parses a Layer type and extracts ROut, E, RIn parameters.
    // Returns nil if the type is not a Layer.
    // The detection strategy is chosen based on the detected Effect version:
    // v4 uses direct symbol lookup, v3/unknown uses property iteration.
    pub fn layer_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Layer> {
        let t = t.into()?;
        cached(
            self,
            |l| &mut l.layer_type,
            t,
            |tp| {
                let version = tp.detect_effect_version();
                if version == EffectMajorVersion::V4 {
                    let variance_struct_type = tp.get_type_of_property_by_name(t, LAYER_TYPE_ID)?;

                    return tp.parse_layer_variance_struct(Some(variance_struct_type));
                }

                // v3 / unknown: iterate properties looking for a layer variance struct
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

                // Sort so properties containing "LayerTypeId" come first (optimization heuristic)
                ets_go::sort::slice_stable(&mut candidates, |i, j| {
                    let i_has = i.name().contains("LayerTypeId");
                    let j_has = j.name().contains("LayerTypeId");
                    i_has && !j_has
                });

                // Try each candidate as a layer variance struct
                for prop in candidates {
                    let prop_type = tp.checker.get_type_of_symbol_at_location(prop, None);
                    if let Some(result) = tp.parse_layer_variance_struct(prop_type) {
                        return Some(result);
                    }
                }

                None
            },
        )
    }

    // go: internal/typeparser/layer_type.go:117 TypeParser.IsLayerType 25c7e6f3e34586ce
    // IsLayerType returns true if the type has the Layer variance struct.
    pub fn is_layer_type(&mut self, t: impl Into<Option<P<Type>>>) -> bool {
        self.layer_type(t).is_some()
    }
}

// go: internal/typeparser/layer_type.go:121 isLayerTypeSourceFile e62644f8d31e8e3d
// Rust: Go's `c` argument is `tp.checker`.
pub(crate) fn is_layer_type_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let Some(module_sym) = get_symbol_of_declaration(tp.checker, sf.as_node()) else {
        return false;
    };

    let Some(layer_sym) =
        tp.checker.try_get_member_in_module_exports_and_properties("Layer", module_sym)
    else {
        return false;
    };

    let layer_type = tp.checker.get_declared_type_of_symbol_exported(layer_sym);

    tp.layer_type(layer_type).is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/layer_type.go:146 TypeParser.IsNodeReferenceToEffectLayerModuleApi ce310a86eb9eca49
    // IsNodeReferenceToEffectLayerModuleApi reports whether node resolves to a member
    // exported by the "effect" package from a module that exports the Layer type.
    pub fn is_node_reference_to_effect_layer_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(node, &EFFECT_LAYER_MODULE_DESCRIPTOR, member_name)
    }
}
