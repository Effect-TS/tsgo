use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/vitest_api.go:5 vitestRunnerPackageDescriptor
pub(crate) static VITEST_RUNNER_PACKAGE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(ModuleDescriptorId::VitestRunner, "@vitest/runner", None);

// go: internal/typeparser/vitest_api.go:7 vitestPackageDescriptor
pub(crate) static VITEST_PACKAGE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(ModuleDescriptorId::Vitest, "vitest", None);

// go: internal/typeparser/vitest_api.go:9 effectVitestPackageDescriptor
pub(crate) static EFFECT_VITEST_PACKAGE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(ModuleDescriptorId::EffectVitest, "@effect/vitest", None);

impl TypeParser<'_> {
    // go: internal/typeparser/vitest_api.go:15 TypeParser.IsNodeReferenceToVitestApi
    // IsNodeReferenceToVitestApi reports whether node resolves to a Vitest API.
    // Vitest re-exports most test APIs from @vitest/runner, while global APIs are
    // declared directly by the vitest package. The package fallback handles globals
    // and Vitest 4 exports whose public names alias minified runner symbols.
    pub fn is_node_reference_to_vitest_api(&mut self, node: P<Node>, member_name: &str) -> bool {
        if self.is_node_reference_to_module_export(
            node,
            &VITEST_RUNNER_PACKAGE_DESCRIPTOR,
            member_name,
        ) || self.is_node_reference_to_module_export(
            node,
            &VITEST_PACKAGE_DESCRIPTOR,
            member_name,
        ) {
            return true;
        }

        if reference_node_name(Some(node)) != member_name {
            return false;
        }
        self.is_node_reference_to_module(node, &VITEST_RUNNER_PACKAGE_DESCRIPTOR)
            || self.is_node_reference_to_module(node, &VITEST_PACKAGE_DESCRIPTOR)
    }

    // go: internal/typeparser/vitest_api.go:30 TypeParser.IsNodeReferenceToEffectVitestApi
    // IsNodeReferenceToEffectVitestApi reports whether node resolves to an API
    // implemented by @effect/vitest rather than re-exported from Vitest.
    pub fn is_node_reference_to_effect_vitest_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_VITEST_PACKAGE_DESCRIPTOR,
            member_name,
        )
    }
}

// go: internal/typeparser/vitest_api.go:34 referenceNodeName
pub(crate) fn reference_node_name(node: Option<P<Node>>) -> &'static str {
    let Some(node) = node else {
        return "";
    };
    match node.kind() {
        Kind::Identifier => node.text(),
        Kind::PropertyAccessExpression => node.as_property_access_expression().name().text(),
        _ => "",
    }
}
