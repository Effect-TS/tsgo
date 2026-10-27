use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/predicate_type.go:8 effectPredicatePackageSourceFileDescriptor a523b87fcd4949ca
pub(crate) static EFFECT_PREDICATE_PACKAGE_SOURCE_FILE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Predicate,
        "effect",
        Some(&is_effect_predicate_source_file),
    );

// go: internal/typeparser/predicate_type.go:8 effectPredicatePackageSourceFileDescriptor a523b87fcd4949ca
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_predicate_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    c.try_get_member_in_module_exports_and_properties("Predicate", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("Refinement", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("hasProperty", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("isTagged", module_sym).is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/predicate_type.go:28 TypeParser.IsNodeReferenceToEffectPredicateModuleApi 473f4014ea5c12c0
    // IsNodeReferenceToEffectPredicateModuleApi reports whether node resolves to a
    // member exported by Effect's Predicate module.
    pub fn is_node_reference_to_effect_predicate_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_PREDICATE_PACKAGE_SOURCE_FILE_DESCRIPTOR,
            member_name,
        )
    }
}
