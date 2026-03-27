package typeparser

import (
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// isSqlModelTypeSourceFile checks if a source file is @effect/sql Model module
// by verifying it exports "Class", "makeRepository", and "makeDataLoaders".
func isSqlModelTypeSourceFile(c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := moduleSymbolFromSourceFile(c, sf)
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
func IsNodeReferenceToEffectSqlModelModuleApi(c *checker.Checker, node *ast.Node, memberName string) bool {
	if c == nil || node == nil {
		return false
	}

	sym := c.GetSymbolAtLocation(node)
	if sym == nil && node.Kind == ast.KindPropertyAccessExpression {
		if prop := node.AsPropertyAccessExpression(); prop != nil && prop.Name() != nil {
			sym = c.GetSymbolAtLocation(prop.Name())
		}
	}
	sym = resolveAliasedSymbol(c, sym)
	if sym == nil {
		return false
	}

	for _, decl := range sym.Declarations {
		if decl == nil {
			continue
		}
		sf := ast.GetSourceFileOfNode(decl)
		if sf == nil {
			continue
		}
		pkg := PackageJsonForSourceFile(c, sf)
		if pkg == nil {
			continue
		}
		if name, ok := pkg.Name.GetValue(); ok && strings.EqualFold(name, "@effect/sql") {
			if !isSqlModelTypeSourceFile(c, sf) {
				continue
			}
			moduleSym := moduleSymbolFromSourceFile(c, sf)
			if moduleSym == nil {
				continue
			}
			exportSym := c.TryGetMemberInModuleExportsAndProperties(memberName, moduleSym)
			if exportSym == nil {
				continue
			}
			exportSym = resolveAliasedSymbol(c, exportSym)
			if checker.Checker_getSymbolIfSameReference(c, exportSym, sym) != nil {
				return true
			}
		}
	}

	return false
}
