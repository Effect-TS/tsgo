package typeparser

import "github.com/microsoft/typescript-go/shim/ast"

var vitestRunnerPackageDescriptor = PackageSourceFileDescriptor{
	PackageName: "@vitest/runner",
}

var vitestPackageDescriptor = PackageSourceFileDescriptor{
	PackageName: "vitest",
}

var effectVitestPackageDescriptor = PackageSourceFileDescriptor{
	PackageName: "@effect/vitest",
}

// IsNodeReferenceToVitestApi reports whether node resolves to a Vitest API.
// Vitest re-exports most test APIs from @vitest/runner, while global APIs are
// declared directly by the vitest package. The package fallback handles globals
// and Vitest 4 exports whose public names alias minified runner symbols.
func (tp *TypeParser) IsNodeReferenceToVitestApi(node *ast.Node, memberName string) bool {
	if tp.IsNodeReferenceToModuleExport(node, vitestRunnerPackageDescriptor, memberName) ||
		tp.IsNodeReferenceToModuleExport(node, vitestPackageDescriptor, memberName) {
		return true
	}

	if referenceNodeName(node) != memberName {
		return false
	}
	return tp.IsNodeReferenceToModule(node, vitestRunnerPackageDescriptor) ||
		tp.IsNodeReferenceToModule(node, vitestPackageDescriptor)
}

// IsNodeReferenceToEffectVitestApi reports whether node resolves to an API
// implemented by @effect/vitest rather than re-exported from Vitest.
func (tp *TypeParser) IsNodeReferenceToEffectVitestApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectVitestPackageDescriptor, memberName)
}

func referenceNodeName(node *ast.Node) string {
	if node == nil {
		return ""
	}
	switch node.Kind {
	case ast.KindIdentifier:
		return node.Text()
	case ast.KindPropertyAccessExpression:
		if name := node.AsPropertyAccessExpression().Name(); name != nil {
			return name.Text()
		}
	}
	return ""
}
