use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/data_type.go:8 effectDataPackageSourceFileDescriptor
pub(crate) static EFFECT_DATA_PACKAGE_SOURCE_FILE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Data,
        "effect",
        Some(&is_effect_data_source_file),
    );

// go: internal/typeparser/data_type.go:8 effectDataPackageSourceFileDescriptor
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_data_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    // The Data module exports "TaggedError"
    let tagged_error_sym =
        c.try_get_member_in_module_exports_and_properties("TaggedError", module_sym);
    if tagged_error_sym.is_none() {
        return false;
    }

    // The Data module also exports "TaggedEnum" (v4) or "taggedEnum" (v3)
    let mut tagged_enum_sym =
        c.try_get_member_in_module_exports_and_properties("TaggedEnum", module_sym);
    if tagged_enum_sym.is_none() {
        tagged_enum_sym =
            c.try_get_member_in_module_exports_and_properties("taggedEnum", module_sym);
    }
    if tagged_enum_sym.is_none() {
        return false;
    }

    true
}

impl TypeParser<'_> {
    // go: internal/typeparser/data_type.go:39 TypeParser.IsNodeReferenceToEffectDataModuleApi
    pub fn is_node_reference_to_effect_data_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_DATA_PACKAGE_SOURCE_FILE_DESCRIPTOR,
            member_name,
        )
    }
}
