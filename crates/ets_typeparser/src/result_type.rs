use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/result_type.go:8 effectResultPackageSourceFileDescriptor 154360679cc7f57b
pub(crate) static EFFECT_RESULT_PACKAGE_SOURCE_FILE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Result,
        "effect",
        Some(&is_effect_result_source_file),
    );

// go: internal/typeparser/result_type.go:8 effectResultPackageSourceFileDescriptor 154360679cc7f57b
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_result_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    // These exports identify the public Result module, which only Effect v4 ships.
    c.try_get_member_in_module_exports_and_properties("Result", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("succeed", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("fail", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("isResult", module_sym).is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/result_type.go:30 TypeParser.IsNodeReferenceToEffectResultModuleApi a54079e4923795b0
    // IsNodeReferenceToEffectResultModuleApi reports whether node resolves to a
    // member exported by Effect's Result module.
    pub fn is_node_reference_to_effect_result_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_RESULT_PACKAGE_SOURCE_FILE_DESCRIPTOR,
            member_name,
        )
    }
}
