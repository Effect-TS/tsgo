package typeparser

import "github.com/microsoft/TypeScript/tsc/shim/ast"

var platformNodePackageDescriptor = newPackageSourceFileDescriptor(moduleDescriptorPlatformNode, "@effect/platform-node", nil)

var platformBunPackageDescriptor = newPackageSourceFileDescriptor(moduleDescriptorPlatformBun, "@effect/platform-bun", nil)

var platformBrowserPackageDescriptor = newPackageSourceFileDescriptor(moduleDescriptorPlatformBrowser, "@effect/platform-browser", nil)

// IsNodeReferenceToPlatformRuntimeApi reports whether node resolves to a member
// exported by @effect/platform-node, @effect/platform-bun or
// @effect/platform-browser, such as `NodeRuntime.runMain`.
func (tp *TypeParser) IsNodeReferenceToPlatformRuntimeApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, platformNodePackageDescriptor, memberName) ||
		tp.IsNodeReferenceToModuleExport(node, platformBunPackageDescriptor, memberName) ||
		tp.IsNodeReferenceToModuleExport(node, platformBrowserPackageDescriptor, memberName)
}
