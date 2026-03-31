package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

var effectDataPackageSourceFileDescriptor = PackageSourceFileDescriptor{
	PackageName: "effect",
	MatchesSourceFile: func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}

		moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		if moduleSym == nil {
			return false
		}

		// The Data module exports "TaggedError"
		taggedErrorSym := c.TryGetMemberInModuleExportsAndProperties("TaggedError", moduleSym)
		if taggedErrorSym == nil {
			return false
		}

		// The Data module also exports "TaggedEnum" (v4) or "taggedEnum" (v3)
		taggedEnumSym := c.TryGetMemberInModuleExportsAndProperties("TaggedEnum", moduleSym)
		if taggedEnumSym == nil {
			taggedEnumSym = c.TryGetMemberInModuleExportsAndProperties("taggedEnum", moduleSym)
		}
		if taggedEnumSym == nil {
			return false
		}

		return true
	},
}

func (tp *TypeParser) IsNodeReferenceToEffectDataModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectDataPackageSourceFileDescriptor, memberName)
}
