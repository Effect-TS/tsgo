package typeparser

import (
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// isEffectModelTypeSourceFile checks if a source file is the effect/unstable/schema Model module
// by verifying it exports "Class", "Generated", and "FieldOption".
// These symbols are chosen to disambiguate Model from Schema (which also exports "Class"),
// matching the TypeScript reference implementation.
func isEffectModelTypeSourceFile(c *checker.Checker, sf *ast.SourceFile) bool {
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
	// Generated is unique to Model, not present in Schema
	if c.TryGetMemberInModuleExportsAndProperties("Generated", moduleSym) == nil {
		return false
	}
	// FieldOption is unique to v4 Model
	if c.TryGetMemberInModuleExportsAndProperties("FieldOption", moduleSym) == nil {
		return false
	}

	return true
}

// IsNodeReferenceToEffectModelModuleApi reports whether node resolves to a member
// exported by the "effect" package from a module that exports the Model API
// (effect/unstable/schema).
func IsNodeReferenceToEffectModelModuleApi(c *checker.Checker, node *ast.Node, memberName string) bool {
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
			if !isEffectModelTypeSourceFile(c, sf) {
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
