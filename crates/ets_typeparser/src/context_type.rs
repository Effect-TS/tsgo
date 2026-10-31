use ets_shim::ast::{Node, SourceFile};
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/context_type.go:8 effectContextPackageSourceFileDescriptor 00898d8f68eb53ff
pub(crate) static EFFECT_CONTEXT_PACKAGE_SOURCE_FILE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Context,
        "effect",
        Some(&is_effect_context_source_file),
    );

// go: internal/typeparser/context_type.go:8 effectContextPackageSourceFileDescriptor 00898d8f68eb53ff
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_context_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let Some(module_sym) = get_symbol_of_declaration(tp.checker, sf.as_node()) else {
        return false;
    };

    // The Context module exports "Context" (the namespace/type)
    let context_sym =
        tp.checker.try_get_member_in_module_exports_and_properties("Context", module_sym);
    if context_sym.is_none() {
        return false;
    }

    // Effect v3 exports Context.Tag, while newer v4 betas export Context.Service.
    if tp.supported_effect_version() == EffectMajorVersion::V4 {
        let service_sym =
            tp.checker.try_get_member_in_module_exports_and_properties("Service", module_sym);
        return service_sym.is_some();
    }
    let tag_sym = tp.checker.try_get_member_in_module_exports_and_properties("Tag", module_sym);
    tag_sym.is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/context_type.go:36 TypeParser.IsNodeReferenceToEffectContextModuleApi f72b2a1da02b3612
    pub fn is_node_reference_to_effect_context_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_CONTEXT_PACKAGE_SOURCE_FILE_DESCRIPTOR,
            member_name,
        )
    }
}

// go: internal/typeparser/context_type.go:41 ContextTypeId 3f65ffbf2ccdc299
// ContextTypeId is the property key for Context's variance struct (v4).
pub const CONTEXT_TYPE_ID: &str = "~effect/Context";

impl TypeParser<'_> {
    // go: internal/typeparser/context_type.go:46 TypeParser.ContextType 6d706117dec53495
    // ContextType returns t itself when it is an Effect v4 Context.Context type,
    // or nil otherwise. It checks for the "~effect/Context" variance struct and its
    // contravariant _Services marker, so any, unknown and never never match.
    pub fn context_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<P<Type>> {
        let t = t.into()?;
        cached(
            self,
            |l| &mut l.context_type,
            t,
            |tp| {
                if tp.detect_effect_version() != EffectMajorVersion::V4 {
                    return None;
                }
                if t.flags().intersects(TypeFlags::Any | TypeFlags::Unknown | TypeFlags::Never) {
                    return None;
                }
                let variance = tp.get_type_of_property_by_name(t, CONTEXT_TYPE_ID);
                if variance.is_none()
                    || tp.extract_contravariant_type(variance, "_Services").is_none()
                {
                    return None;
                }
                Some(t)
            },
        )
    }

    // go: internal/typeparser/context_type.go:67 TypeParser.ContextTypeAtLocation 548b56741da3da46
    // ContextTypeAtLocation parses the type of node as a v4 Context type, without
    // computing the type when nodeCannotBeEffectLibraryType rules node out.
    pub fn context_type_at_location(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<P<Type>> {
        let t = self.effect_library_type_at_location(node);
        self.context_type(t)
    }
}
