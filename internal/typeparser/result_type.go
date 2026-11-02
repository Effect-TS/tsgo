package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

var effectResultPackageSourceFileDescriptor = newPackageSourceFileDescriptor(moduleDescriptorResult,
	"effect",
	func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}

		moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		if moduleSym == nil {
			return false
		}

		// isResult keeps out Exit, which shares isSuccess, isFailure and match,
		// while the guards and match keep out Workflow, which exports isResult.
		return c.TryGetMemberInModuleExportsAndProperties("Result", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isResult", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isSuccess", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isFailure", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("match", moduleSym) != nil
	},
)

// IsNodeReferenceToEffectResultModuleApi reports whether node resolves to a
// member exported by Effect's Result module.
func (tp *TypeParser) IsNodeReferenceToEffectResultModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectResultPackageSourceFileDescriptor, memberName)
}
