use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/exit_type.go:8 effectExitPackageSourceFileDescriptor 48dead51ba2fe1ba
pub(crate) static EFFECT_EXIT_PACKAGE_SOURCE_FILE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Exit,
        "effect",
        Some(&is_effect_exit_source_file),
    );

// go: internal/typeparser/exit_type.go:8 effectExitPackageSourceFileDescriptor 48dead51ba2fe1ba
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_exit_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    // isExit keeps out Result, which shares isSuccess and isFailure.
    c.try_get_member_in_module_exports_and_properties("Exit", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("isExit", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("isSuccess", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("isFailure", module_sym).is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/exit_type.go:28 TypeParser.IsNodeReferenceToEffectExitModuleApi f739106781fc16e0
    // IsNodeReferenceToEffectExitModuleApi reports whether node resolves to a
    // member exported by Effect's Exit module.
    pub fn is_node_reference_to_effect_exit_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_EXIT_PACKAGE_SOURCE_FILE_DESCRIPTOR,
            member_name,
        )
    }

    // go: internal/typeparser/exit_type.go:33 TypeParser.IsExpressionEffectExitModule 4cc5cd3a433c0d24
    // IsExpressionEffectExitModule reports whether node resolves to the Exit module namespace.
    pub fn is_expression_effect_exit_module(&mut self, node: P<Node>) -> bool {
        self.is_node_reference_to_module(node, &EFFECT_EXIT_PACKAGE_SOURCE_FILE_DESCRIPTOR)
    }
}
