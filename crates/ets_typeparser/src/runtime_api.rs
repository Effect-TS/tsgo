use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/runtime_api.go:11 effectRuntimeModuleDescriptor 61f3d216defe7b91
// effectRuntimeModuleDescriptor matches Effect v3's Runtime module, the one
// that runs effects against a Runtime value. Effect v4's Runtime module only
// exports teardown and runMain helpers, so it does not match.
pub(crate) static EFFECT_RUNTIME_MODULE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Runtime,
        "effect",
        Some(&is_effect_runtime_source_file),
    );

// go: internal/typeparser/runtime_api.go:11 effectRuntimeModuleDescriptor 61f3d216defe7b91
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_runtime_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };
    c.try_get_member_in_module_exports_and_properties("Runtime", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("runPromise", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("runFork", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("defaultRuntime", module).is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/runtime_api.go:26 TypeParser.IsNodeReferenceToEffectRuntimeModuleApi 001d695bf8fb61db
    pub fn is_node_reference_to_effect_runtime_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_RUNTIME_MODULE_DESCRIPTOR,
            member_name,
        )
    }

    // go: internal/typeparser/runtime_api.go:32 TypeParser.IsExpressionEffectRuntimeModule f0d7a7fb6dc862a6
    // IsExpressionEffectRuntimeModule reports whether node resolves to Effect v3's
    // Runtime module namespace itself.
    pub fn is_expression_effect_runtime_module(&mut self, node: P<Node>) -> bool {
        self.is_node_reference_to_module(node, &EFFECT_RUNTIME_MODULE_DESCRIPTOR)
    }
}
