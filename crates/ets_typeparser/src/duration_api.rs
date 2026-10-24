use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/duration_api.go:8 effectDurationModuleDescriptor
pub(crate) static EFFECT_DURATION_MODULE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Duration,
        "effect",
        Some(&is_effect_duration_source_file),
    );

// go: internal/typeparser/duration_api.go:8 effectDurationModuleDescriptor
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_duration_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };
    c.try_get_member_in_module_exports_and_properties("Duration", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("zero", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("millis", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("isDuration", module).is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/duration_api.go:23 TypeParser.IsNodeReferenceToEffectDurationModuleApi
    pub fn is_node_reference_to_effect_duration_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_DURATION_MODULE_DESCRIPTOR,
            member_name,
        )
    }

    // go: internal/typeparser/duration_api.go:27 TypeParser.IsExpressionEffectDurationModule
    pub fn is_expression_effect_duration_module(&mut self, node: P<Node>) -> bool {
        self.is_node_reference_to_module(node, &EFFECT_DURATION_MODULE_DESCRIPTOR)
    }
}
