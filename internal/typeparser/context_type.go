package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

var effectContextPackageSourceFileDescriptor = PackageSourceFileDescriptor{
	PackageName:       "effect",
	MatchesSourceFile: isContextTypeSourceFile,
}

// isContextTypeSourceFile checks if a source file is the Context module
// by verifying it exports both "Context" and "Tag".
func isContextTypeSourceFile(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
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
}

// IsNodeReferenceToEffectContextModuleApi reports whether node resolves to a member
// exported by the "effect" package from a module that exports the Context type.
func (tp *TypeParser) IsNodeReferenceToEffectContextModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectContextPackageSourceFileDescriptor, memberName)
}
