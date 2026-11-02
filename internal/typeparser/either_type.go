package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

var effectEitherPackageSourceFileDescriptor = newPackageSourceFileDescriptor(moduleDescriptorEither,
	"effect",
	func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}

		moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		if moduleSym == nil {
			return false
		}

		// These exports identify the public Either module, which only Effect v3 ships.
		return c.TryGetMemberInModuleExportsAndProperties("Either", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("right", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("left", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isEither", moduleSym) != nil
	},
)

// IsNodeReferenceToEffectEitherModuleApi reports whether node resolves to a
// member exported by Effect's Either module.
func (tp *TypeParser) IsNodeReferenceToEffectEitherModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectEitherPackageSourceFileDescriptor, memberName)
}
