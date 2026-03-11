package typeparser

import (
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// isContextTypeSourceFile checks if a source file is the Context module
// by verifying it exports both "Context" and "Tag".
func isContextTypeSourceFile(c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := moduleSymbolFromSourceFile(c, sf)
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
func IsNodeReferenceToEffectContextModuleApi(c *checker.Checker, node *ast.Node, memberName string) bool {
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
			if !isContextTypeSourceFile(c, sf) {
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
			if symbolsMatch(c, exportSym, sym) {
				return true
			}
		}
	}

	return false
}
