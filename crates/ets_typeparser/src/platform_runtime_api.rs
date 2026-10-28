use ets_shim::ast::Node;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/platform_runtime_api.go:5 platformNodePackageDescriptor 9bdc7a0e29cc0727
pub(crate) static PLATFORM_NODE_PACKAGE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::PlatformNode,
        "@effect/platform-node",
        None,
    );

// go: internal/typeparser/platform_runtime_api.go:7 platformBunPackageDescriptor 92812a5758813759
pub(crate) static PLATFORM_BUN_PACKAGE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::PlatformBun,
        "@effect/platform-bun",
        None,
    );

// go: internal/typeparser/platform_runtime_api.go:9 platformBrowserPackageDescriptor 3200c073d42f1dcd
pub(crate) static PLATFORM_BROWSER_PACKAGE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::PlatformBrowser,
        "@effect/platform-browser",
        None,
    );

impl TypeParser<'_> {
    // go: internal/typeparser/platform_runtime_api.go:14 TypeParser.IsNodeReferenceToPlatformRuntimeApi af5b296e1827c8a1
    // IsNodeReferenceToPlatformRuntimeApi reports whether node resolves to a member
    // exported by the @effect/platform-node, @effect/platform-bun or
    // @effect/platform-browser packages, such as `NodeRuntime.runMain`.
    pub fn is_node_reference_to_platform_runtime_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &PLATFORM_NODE_PACKAGE_DESCRIPTOR,
            member_name,
        ) || self.is_node_reference_to_module_export(
            node,
            &PLATFORM_BUN_PACKAGE_DESCRIPTOR,
            member_name,
        ) || self.is_node_reference_to_module_export(
            node,
            &PLATFORM_BROWSER_PACKAGE_DESCRIPTOR,
            member_name,
        )
    }
}
