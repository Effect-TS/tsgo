use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/context_type.go:8 effectContextPackageSourceFileDescriptor
pub(crate) static EFFECT_CONTEXT_PACKAGE_SOURCE_FILE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Context,
        "effect",
        Some(&is_effect_context_source_file),
    );

// go: internal/typeparser/context_type.go:8 effectContextPackageSourceFileDescriptor
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
    // go: internal/typeparser/context_type.go:36 TypeParser.IsNodeReferenceToEffectContextModuleApi
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
