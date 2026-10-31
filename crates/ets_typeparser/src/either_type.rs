use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/either_type.go:8 effectEitherPackageSourceFileDescriptor 92c13262209b98bc
pub(crate) static EFFECT_EITHER_PACKAGE_SOURCE_FILE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Either,
        "effect",
        Some(&is_effect_either_source_file),
    );

// go: internal/typeparser/either_type.go:8 effectEitherPackageSourceFileDescriptor 92c13262209b98bc
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_either_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    // These exports identify the public Either module, which only Effect v3 ships.
    c.try_get_member_in_module_exports_and_properties("Either", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("right", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("left", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("isEither", module_sym).is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/either_type.go:30 TypeParser.IsNodeReferenceToEffectEitherModuleApi 6f7d794cd56f981f
    // IsNodeReferenceToEffectEitherModuleApi reports whether node resolves to a
    // member exported by Effect's Either module.
    pub fn is_node_reference_to_effect_either_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_EITHER_PACKAGE_SOURCE_FILE_DESCRIPTOR,
            member_name,
        )
    }
}
