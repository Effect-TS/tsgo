use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/metric_api.go:8 effectMetricModuleDescriptor b00521599e91c7be
pub(crate) static EFFECT_METRIC_MODULE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Metric,
        "effect",
        Some(&is_effect_metric_source_file),
    );

// go: internal/typeparser/metric_api.go:8 effectMetricModuleDescriptor b00521599e91c7be
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_metric_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };
    c.try_get_member_in_module_exports_and_properties("Metric", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("update", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("counter", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("gauge", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("withConstantInput", module).is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/metric_api.go:26 TypeParser.IsNodeReferenceToEffectMetricModuleApi 9f23fc6141c845cd
    // IsNodeReferenceToEffectMetricModuleApi reports whether node resolves to a
    // member exported by Effect's Metric module.
    pub fn is_node_reference_to_effect_metric_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(node, &EFFECT_METRIC_MODULE_DESCRIPTOR, member_name)
    }

    // go: internal/typeparser/metric_api.go:31 TypeParser.IsExpressionEffectMetricModule 81529c11717423ff
    // IsExpressionEffectMetricModule reports whether node references Effect's Metric module.
    pub fn is_expression_effect_metric_module(&mut self, node: P<Node>) -> bool {
        self.is_node_reference_to_module(node, &EFFECT_METRIC_MODULE_DESCRIPTOR)
    }
}
