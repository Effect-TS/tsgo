package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// effectRuntimeModuleDescriptor matches Effect v3's Runtime module, the one
// that runs effects against a Runtime value. Effect v4's Runtime module only
// exports teardown and runMain helpers, so it does not match.
var effectRuntimeModuleDescriptor = newPackageSourceFileDescriptor(moduleDescriptorRuntime,
	"effect",
	func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}
		module := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		return module != nil &&
			c.TryGetMemberInModuleExportsAndProperties("Runtime", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("runPromise", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("runFork", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("defaultRuntime", module) != nil
	},
)

func (tp *TypeParser) IsNodeReferenceToEffectRuntimeModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectRuntimeModuleDescriptor, memberName)
}

// IsExpressionEffectRuntimeModule reports whether node resolves to Effect v3's
// Runtime module namespace itself.
func (tp *TypeParser) IsExpressionEffectRuntimeModule(node *ast.Node) bool {
	return tp.IsNodeReferenceToModule(node, effectRuntimeModuleDescriptor)
}
