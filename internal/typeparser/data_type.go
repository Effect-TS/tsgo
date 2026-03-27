package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

var effectDataPackageSourceFileDescriptor = PackageSourceFileDescriptor{
	PackageName:       "effect",
	MatchesSourceFile: isDataTypeSourceFile,
}

// isDataTypeSourceFile checks if a source file is the Data module
// by verifying it exports both "TaggedError" and either "TaggedEnum" or "taggedEnum".
func isDataTypeSourceFile(c *checker.Checker, sf *ast.SourceFile) bool {
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
}

// IsNodeReferenceToEffectDataModuleApi reports whether node resolves to a member
// exported by the "effect" package from a module that exports the Data type.
func IsNodeReferenceToEffectDataModuleApi(c *checker.Checker, node *ast.Node, memberName string) bool {
	return IsNodeReferenceToModuleExport(c, node, effectDataPackageSourceFileDescriptor, memberName)
}
