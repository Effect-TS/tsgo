package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

var effectExitPackageSourceFileDescriptor = newPackageSourceFileDescriptor(moduleDescriptorExit,
	"effect",
	func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}
		moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		if moduleSym == nil {
			return false
		}
		// isExit keeps out Result, which shares isSuccess and isFailure.
		return c.TryGetMemberInModuleExportsAndProperties("Exit", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isExit", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isSuccess", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isFailure", moduleSym) != nil
	},
)

// IsNodeReferenceToEffectExitModuleApi reports whether node resolves to a
// member exported by Effect's Exit module.
func (tp *TypeParser) IsNodeReferenceToEffectExitModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectExitPackageSourceFileDescriptor, memberName)
}

// IsExpressionEffectExitModule reports whether node resolves to the Exit module namespace.
func (tp *TypeParser) IsExpressionEffectExitModule(node *ast.Node) bool {
	return tp.IsNodeReferenceToModule(node, effectExitPackageSourceFileDescriptor)
}
