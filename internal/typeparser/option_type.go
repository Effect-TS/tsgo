package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

var effectOptionPackageSourceFileDescriptor = newPackageSourceFileDescriptor(
	"effect",
	func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}

		moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		if moduleSym == nil {
			return false
		}

		// These exports identify the public Option module in both Effect v3 and v4.
		return c.TryGetMemberInModuleExportsAndProperties("Option", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("some", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("none", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isOption", moduleSym) != nil
	},
)

// IsNodeReferenceToEffectOptionModuleApi reports whether node resolves to a
// member exported by Effect's Option module.
func (tp *TypeParser) IsNodeReferenceToEffectOptionModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectOptionPackageSourceFileDescriptor, memberName)
}
