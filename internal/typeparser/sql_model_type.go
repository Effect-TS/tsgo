package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

var sqlModelModuleDescriptor = PackageSourceFileDescriptor{
	PackageName:       "@effect/sql",
	MatchesSourceFile: isSqlModelTypeSourceFile,
}

// isSqlModelTypeSourceFile checks if a source file is @effect/sql Model module
// by verifying it exports "Class", "makeRepository", and "makeDataLoaders".
func isSqlModelTypeSourceFile(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
	if moduleSym == nil {
		return false
	}

	if c.TryGetMemberInModuleExportsAndProperties("Class", moduleSym) == nil {
		return false
	}
	if c.TryGetMemberInModuleExportsAndProperties("makeRepository", moduleSym) == nil {
		return false
	}
	if c.TryGetMemberInModuleExportsAndProperties("makeDataLoaders", moduleSym) == nil {
		return false
	}

	return true
}

// IsNodeReferenceToEffectSqlModelModuleApi reports whether node resolves to a member
// exported by the "@effect/sql" package from a module that exports the Model API.
func (tp *TypeParser) IsNodeReferenceToEffectSqlModelModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, sqlModelModuleDescriptor, memberName)
}
