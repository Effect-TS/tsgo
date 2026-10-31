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

		// These exports identify the public Result module, which only Effect v4 ships.
		return c.TryGetMemberInModuleExportsAndProperties("Result", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("succeed", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("fail", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isResult", moduleSym) != nil
	},
)

// IsNodeReferenceToEffectResultModuleApi reports whether node resolves to a
// member exported by Effect's Result module.
func (tp *TypeParser) IsNodeReferenceToEffectResultModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectResultPackageSourceFileDescriptor, memberName)
}
