use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/function_module.go:8 effectFunctionPackageSourceFileDescriptor
pub(crate) static EFFECT_FUNCTION_PACKAGE_SOURCE_FILE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Function,
        "effect",
        Some(&is_effect_function_source_file),
    );

// go: internal/typeparser/function_module.go:8 effectFunctionPackageSourceFileDescriptor
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_function_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    c.try_get_member_in_module_exports_and_properties("identity", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("dual", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("pipe", module_sym).is_some()
        && c.try_get_member_in_module_exports_and_properties("flow", module_sym).is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/function_module.go:28 TypeParser.IsNodeReferenceToEffectFunctionModuleApi
    // IsNodeReferenceToEffectFunctionModuleApi reports whether node resolves to a
    // member exported by Effect's Function module.
    pub fn is_node_reference_to_effect_function_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_FUNCTION_PACKAGE_SOURCE_FILE_DESCRIPTOR,
            member_name,
        )
    }

    // go: internal/typeparser/function_module.go:35 TypeParser.IsNodeDirectReferenceToEffectFunctionModuleApi
    // IsNodeDirectReferenceToEffectFunctionModuleApi is like
    // IsNodeReferenceToEffectFunctionModuleApi but does not follow local variable
    // aliases: node must name the export itself, through imports and re-exports only.
    pub fn is_node_direct_reference_to_effect_function_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        let symbol_at_location = self.get_symbol_at_location(node);
        let sym = self.resolve_aliased_symbol(symbol_at_location);
        sym.is_some_and(|sym| {
            self.symbol_has_declaration_in_module_cached(
                sym,
                &EFFECT_FUNCTION_PACKAGE_SOURCE_FILE_DESCRIPTOR,
            )
        }) && self.is_node_reference_to_effect_function_module_api(node, member_name)
    }
}
