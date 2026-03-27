package typeparser

import (
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// isDataTypeSourceFile checks if a source file is the Data module
// by verifying it exports both "TaggedError" and either "TaggedEnum" or "taggedEnum".
func isDataTypeSourceFile(c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := moduleSymbolFromSourceFile(c, sf)
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
		if name, ok := pkg.Name.GetValue(); ok && strings.EqualFold(name, "effect") {
			if !isDataTypeSourceFile(c, sf) {
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
