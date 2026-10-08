package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

var effectPredicatePackageSourceFileDescriptor = newPackageSourceFileDescriptor(
	"effect",
	func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}
		moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		if moduleSym == nil {
			return false
		}

		return c.TryGetMemberInModuleExportsAndProperties("Predicate", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("Refinement", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("hasProperty", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isTagged", moduleSym) != nil
	},
)

// IsNodeReferenceToEffectPredicateModuleApi reports whether node resolves to a
// member exported by Effect's Predicate module.
func (tp *TypeParser) IsNodeReferenceToEffectPredicateModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectPredicatePackageSourceFileDescriptor, memberName)
}
