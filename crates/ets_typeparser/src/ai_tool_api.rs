use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/ai_tool_api.go:8 effectAiToolModuleDescriptor 1bbf62afb77a2e56
pub(crate) static EFFECT_AI_TOOL_MODULE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::AiTool,
        "effect",
        Some(&is_effect_ai_tool_source_file),
    );

// go: internal/typeparser/ai_tool_api.go:8 effectAiToolModuleDescriptor 1bbf62afb77a2e56
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_ai_tool_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };
    c.try_get_member_in_module_exports_and_properties("make", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("dynamic", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("providerDefined", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("EmptyParams", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("isUserDefined", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("getJsonSchema", module).is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/ai_tool_api.go:27 TypeParser.IsNodeReferenceToEffectAiToolModuleApi a9bd85ecb1a5bc30
    // IsNodeReferenceToEffectAiToolModuleApi reports whether node resolves to a
    // member exported by Effect's AI Tool module.
    pub fn is_node_reference_to_effect_ai_tool_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_AI_TOOL_MODULE_DESCRIPTOR,
            member_name,
        )
    }

    // go: internal/typeparser/ai_tool_api.go:32 TypeParser.IsExpressionEffectAiToolModule 4d88c4e23b166bf1
    // IsExpressionEffectAiToolModule reports whether node references Effect's AI Tool module.
    pub fn is_expression_effect_ai_tool_module(&mut self, node: P<Node>) -> bool {
        self.is_node_reference_to_module(node, &EFFECT_AI_TOOL_MODULE_DESCRIPTOR)
    }
}
