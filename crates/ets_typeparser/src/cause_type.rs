use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/cause_type.go:8 effectCausePackageSourceFileDescriptor
pub(crate) static EFFECT_CAUSE_PACKAGE_SOURCE_FILE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Cause,
        "effect",
        Some(&is_effect_cause_source_file),
    );

// go: internal/typeparser/cause_type.go:8 effectCausePackageSourceFileDescriptor
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_cause_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    c.try_get_member_in_module_exports_and_properties("Cause", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("isCause", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("NoSuchElementError", module_sym)
            .is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/cause_type.go:28 TypeParser.IsNodeReferenceToEffectCauseModuleApi
    // IsNodeReferenceToEffectCauseModuleApi reports whether node resolves to a
    // member exported by Effect's Cause module.
    pub fn is_node_reference_to_effect_cause_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_CAUSE_PACKAGE_SOURCE_FILE_DESCRIPTOR,
            member_name,
        )
    }
}
