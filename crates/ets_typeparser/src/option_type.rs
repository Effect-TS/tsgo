use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/option_type.go:8 effectOptionPackageSourceFileDescriptor 6191cf203f90a873
pub(crate) static EFFECT_OPTION_PACKAGE_SOURCE_FILE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Option,
        "effect",
        Some(&is_effect_option_source_file),
    );

// go: internal/typeparser/option_type.go:8 effectOptionPackageSourceFileDescriptor 6191cf203f90a873
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_option_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    // These exports identify the public Option module in both Effect v3 and v4.
    c.try_get_member_in_module_exports_and_properties("Option", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("some", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("none", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("isOption", module_sym).is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/option_type.go:30 TypeParser.IsNodeReferenceToEffectOptionModuleApi 7b3b9e6db7e6fe54
    // IsNodeReferenceToEffectOptionModuleApi reports whether node resolves to a
    // member exported by Effect's Option module.
    pub fn is_node_reference_to_effect_option_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_OPTION_PACKAGE_SOURCE_FILE_DESCRIPTOR,
            member_name,
        )
    }
}
