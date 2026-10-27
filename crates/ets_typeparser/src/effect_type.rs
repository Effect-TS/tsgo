use ets_shim::ast::{Node, SourceFile, Symbol, SymbolFlags};
use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/effect_type.go:11 effectModuleDescriptor
pub(crate) static EFFECT_MODULE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Effect,
        "effect",
        Some(&is_effect_type_source_file),
    );

// go: internal/typeparser/effect_type.go:13 effectPackageExportDescriptor
pub(crate) static EFFECT_PACKAGE_EXPORT_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(ModuleDescriptorId::EffectPackage, "effect", None);

// go: internal/typeparser/effect_type.go:17 EffectTypeId
// EffectTypeId is the property key for Effect's variance struct.
// Effect v4 (effect-smol) uses this pattern to encode type parameters.
pub const EFFECT_TYPE_ID: &str = "~effect/Effect";

// go: internal/typeparser/effect_type.go:20 Effect
// Effect represents parsed Effect<A, E, R> type parameters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Effect {
    pub(crate) a: P<Type>, // Success type
    pub(crate) e: P<Type>, // Error type
    pub(crate) r: P<Type>, // Requirements type
}

impl Effect {
    // go: internal/typeparser/effect_type.go:26 Effect.A
    pub fn a(&self) -> P<Type> {
        self.a
    }

    // go: internal/typeparser/effect_type.go:28 Effect.E
    pub fn e(&self) -> P<Type> {
        self.e
    }

    // go: internal/typeparser/effect_type.go:30 Effect.R
    pub fn r(&self) -> P<Type> {
        self.r
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_type.go:36 TypeParser.EffectType
    // EffectType parses an Effect type and extracts A, E, R parameters.
    // Returns nil if the type is not an Effect.
    // The detection strategy is chosen based on the detected Effect version:
    // v4 uses direct symbol lookup, v3/unknown uses property iteration.
    pub fn effect_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Effect> {
        let t = t.into()?;
        cached(
            self,
            |l| &mut l.effect_type,
            t,
            |tp| {
                let version = tp.detect_effect_version();
                if version == EffectMajorVersion::V4 {
                    let variance_struct_type =
                        tp.get_type_of_property_by_name(t, EFFECT_TYPE_ID)?;

                    // Parse the variance struct to extract A, E, R
                    return tp.parse_variance_struct(Some(variance_struct_type));
                }

                // v3 / unknown: iterate properties looking for a variance struct
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

                // Sort so properties containing "EffectTypeId" come first (optimization heuristic)
                ets_go::sort::slice_stable(&mut candidates, |i, j| {
                    let i_has = i.name().contains("EffectTypeId");
                    let j_has = j.name().contains("EffectTypeId");
                    i_has && !j_has
                });

                // Try each candidate as a variance struct
                for prop in candidates {
                    let prop_type = tp.checker.get_type_of_symbol_at_location(prop, None);
                    if let Some(result) = tp.parse_variance_struct(prop_type) {
                        return Some(result);
                    }
                }

                None
            },
        )
    }

    // go: internal/typeparser/effect_type.go:100 TypeParser.parseVarianceStruct
    // parseVarianceStruct extracts A, E, R from a variance struct type.
    pub(crate) fn parse_variance_struct(&mut self, t: Option<P<Type>>) -> Option<Effect> {
        let a = self.extract_covariant_type(t, "_A")?;

        let e = self.extract_covariant_type(t, "_E")?;

        let r = self.extract_covariant_type(t, "_R")?;

        Some(Effect { a, e, r })
    }

    // go: internal/typeparser/effect_type.go:120 TypeParser.IsEffectType
    // IsEffectType returns true if the type has the Effect variance struct.
    pub fn is_effect_type(&mut self, t: impl Into<Option<P<Type>>>) -> bool {
        self.effect_type(t).is_some()
    }

    // go: internal/typeparser/effect_type.go:127 TypeParser.StrictEffectType
    // StrictEffectType returns the parsed Effect type only if the type's symbol name
    // is "Effect". This filters out types like Stream, Layer, HttpApp.Default that
    // carry the variance struct but are not Effect itself.
    pub fn strict_effect_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Effect> {
        let t = t.into()?;
        // Checked before the cache so types of other names leave no entry.
        let sym = t.symbol()?;
        if sym.name() != "Effect" {
            return None;
        }
        cached(self, |l| &mut l.strict_effect_type, t, |tp| tp.effect_type(t))
    }

    // go: internal/typeparser/effect_type.go:143 TypeParser.StrictIsEffectType
    // StrictIsEffectType returns true if the type has the Effect variance struct
    // AND the type's symbol name is "Effect". This filters out types like Stream,
    // Layer, HttpApp.Default that carry the variance struct but are not Effect itself.
    pub fn strict_is_effect_type(&mut self, t: impl Into<Option<P<Type>>>) -> bool {
        self.strict_effect_type(t).is_some()
    }

    // go: internal/typeparser/effect_type.go:149 TypeParser.EffectSubtype
    // EffectSubtype detects types that have the Effect variance struct AND a "_tag" or "get"
    // marker property (e.g., Exit, Option, Either, Pool). Returns nil if not an Effect subtype.
    pub fn effect_subtype(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Effect> {
        let t = t.into()?;
        cached(
            self,
            |l| &mut l.effect_subtype,
            t,
            |tp| {
                // Check for "_tag" or "get" property first (quick rejection)
                let tag_symbol = tp.checker.get_property_of_type_exported(t, "_tag");
                let get_symbol = tp.checker.get_property_of_type_exported(t, "get");
                if tag_symbol.is_none() && get_symbol.is_none() {
                    return None;
                }
                // Must also be an Effect type
                tp.effect_type(t)
            },
        )
    }

    // go: internal/typeparser/effect_type.go:166 TypeParser.IsEffectSubtype
    // IsEffectSubtype returns true if the type is an Effect subtype (has variance struct + "_tag" or "get").
    pub fn is_effect_subtype(&mut self, t: impl Into<Option<P<Type>>>) -> bool {
        self.effect_subtype(t).is_some()
    }

    // go: internal/typeparser/effect_type.go:172 TypeParser.FiberType
    // FiberType detects types that have the Effect variance struct AND both "await" and "poll"
    // properties. Returns nil if the type is not a Fiber.
    pub fn fiber_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<Effect> {
        let t = t.into()?;
        cached(
            self,
            |l| &mut l.fiber_type,
            t,
            |tp| {
                // Check for both "await" and "poll" properties (quick rejection)
                let await_symbol = tp.checker.get_property_of_type_exported(t, "await");
                let poll_symbol = tp.checker.get_property_of_type_exported(t, "poll");
                if await_symbol.is_none() || poll_symbol.is_none() {
                    return None;
                }
                // Must also be an Effect type
                tp.effect_type(t)
            },
        )
    }

    // go: internal/typeparser/effect_type.go:189 TypeParser.IsFiberType
    // IsFiberType returns true if the type is a Fiber type (has variance struct + "await" and "poll").
    pub fn is_fiber_type(&mut self, t: impl Into<Option<P<Type>>>) -> bool {
        self.fiber_type(t).is_some()
    }

    // go: internal/typeparser/effect_type.go:196 TypeParser.HasEffectTypeId
    // HasEffectTypeId returns true if the type has the Effect type identifier.
    // For v4, this is a quick check for the "~effect/Effect" property.
    // For v3/unknown, this defers to IsEffectType since there is no single property shortcut.
    pub fn has_effect_type_id(&mut self, t: impl Into<Option<P<Type>>>) -> bool {
        let Some(t) = t.into() else {
            return false;
        };
        cached(
            self,
            |l| &mut l.has_effect_type_id,
            t,
            |tp| {
                let version = tp.detect_effect_version();
                if version == EffectMajorVersion::V4 {
                    return tp.get_type_of_property_by_name(t, EFFECT_TYPE_ID).is_some();
                }
                // For v3/unknown, the quick check is not available; defer to full detection.
                tp.is_effect_type(t)
            },
        )
    }
}

// go: internal/typeparser/effect_type.go:210 isEffectTypeSourceFile
// Rust: Go's `c` argument is `tp.checker`.
pub(crate) fn is_effect_type_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let Some(module_sym) = get_symbol_of_declaration(tp.checker, sf.as_node()) else {
        return false;
    };

    let Some(effect_sym) =
        tp.checker.try_get_member_in_module_exports_and_properties("Effect", module_sym)
    else {
        return false;
    };

    let effect_type = tp.checker.get_declared_type_of_symbol_exported(effect_sym);

    tp.effect_type(effect_type).is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_type.go:235 TypeParser.IsExpressionEffectModule
    // IsExpressionEffectModule reports whether node resolves to the Effect module namespace
    // (e.g., the `Effect` in `import { Effect } from "effect"`).
    pub fn is_expression_effect_module(&mut self, node: impl Into<Option<P<Node>>>) -> bool {
        self.is_node_reference_to_module(node, &EFFECT_MODULE_DESCRIPTOR)
    }

    // go: internal/typeparser/effect_type.go:243 TypeParser.EffectModuleIdentifierAt
    // EffectModuleIdentifierAt returns the name FindEffectModuleIdentifier picks
    // for the Effect module when that name resolves to the Effect module at
    // location, or "" when no such value binding is in scope there. Type-only
    // bindings, directly or through a type-only re-export, are not values.
    //
    // Rust: Go's nil parser and checker checks have no counterpart (`&mut self` holds the checker).
    pub fn effect_module_identifier_at(&mut self, location: impl Into<Option<P<Node>>>) -> String {
        let Some(location) = location.into() else {
            return String::new();
        };
        let name = find_effect_module_identifier(ets_shim::ast::get_source_file_of_node(location));
        let local = self.checker.resolve_name_exported(&name, location, SymbolFlags::Value, false);
        let Some(local) = local else {
            return String::new();
        };
        if self.checker.get_type_only_alias_declaration_exported(local).is_some() {
            return String::new();
        }
        let sym = self.resolve_aliased_symbol(Some(local));
        let Some(sym) = sym else {
            return String::new();
        };
        if !self.symbol_has_declaration_in_module_cached(sym, &EFFECT_MODULE_DESCRIPTOR) {
            return String::new();
        }
        name
    }

    // go: internal/typeparser/effect_type.go:260 TypeParser.IsNodeReferenceToEffectModuleApi
    // IsNodeReferenceToEffectModuleApi reports whether node resolves to a member exported by the "effect" package.
    pub fn is_node_reference_to_effect_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(node, &EFFECT_MODULE_DESCRIPTOR, member_name)
    }

    // go: internal/typeparser/effect_type.go:269 TypeParser.IsNodeReferenceToEffectPackageExport
    // IsNodeReferenceToEffectPackageExport reports whether node resolves to a member
    // exported by any module in the "effect" npm package. Unlike IsNodeReferenceToEffectModuleApi,
    // this does not require the source file to export the Effect type — it only checks
    // that the declaration lives inside the "effect" package and matches the named export.
    // This is needed for functions like `pipe` which are exported from `effect/Function`.
    pub fn is_node_reference_to_effect_package_export(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_PACKAGE_EXPORT_DESCRIPTOR,
            member_name,
        )
    }
}
