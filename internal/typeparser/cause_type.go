package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

var effectCausePackageSourceFileDescriptor = newPackageSourceFileDescriptor(
	"effect",
	func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}

		moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		if moduleSym == nil {
			return false
		}

		return c.TryGetMemberInModuleExportsAndProperties("Cause", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isCause", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("NoSuchElementError", moduleSym) != nil
	},
)

// IsNodeReferenceToEffectCauseModuleApi reports whether node resolves to a
// member exported by Effect's Cause module.
func (tp *TypeParser) IsNodeReferenceToEffectCauseModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectCausePackageSourceFileDescriptor, memberName)
}
