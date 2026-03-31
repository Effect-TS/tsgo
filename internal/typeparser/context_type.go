package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

var effectContextPackageSourceFileDescriptor = PackageSourceFileDescriptor{
	PackageName: "effect",
	MatchesSourceFile: func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}

		moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		if moduleSym == nil {
			return false
		}

		// The Context module exports "Context" (the namespace/type)
		contextSym := c.TryGetMemberInModuleExportsAndProperties("Context", moduleSym)
		if contextSym == nil {
			return false
		}

		// The Context module also exports "Tag"
		tagSym := c.TryGetMemberInModuleExportsAndProperties("Tag", moduleSym)
		return tagSym != nil
	},
}

func (tp *TypeParser) IsNodeReferenceToEffectContextModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectContextPackageSourceFileDescriptor, memberName)
}
