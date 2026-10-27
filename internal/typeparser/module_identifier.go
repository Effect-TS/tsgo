package typeparser

import (
	"slices"
	"strings"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// ModuleReference is the path of names that reaches a module from a file's
// imports, for example ["Effect"] or ["X", "Effect"] for import * as X from "effect".
type ModuleReference []string

// String joins the path with ".", as it is written in source code.
func (r ModuleReference) String() string {
	return strings.Join(r, ".")
}

// Append returns a new reference that extends r with names, leaving r untouched.
func (r ModuleReference) Append(names ...string) ModuleReference {
	return slices.Concat(r, names)
}

// MatchesNode reports whether node spells out the reference: an identifier, a
// property access chain or a qualified name with the same names.
func (r ModuleReference) MatchesNode(node *ast.Node) bool {
	for i := len(r) - 1; i > 0; i-- {
		var name *ast.Node
		switch node.Kind {
		case ast.KindPropertyAccessExpression:
			access := node.AsPropertyAccessExpression()
			node, name = access.Expression, access.Name()
		case ast.KindQualifiedName:
			qualified := node.AsQualifiedName()
			node, name = qualified.Left, qualified.Right
		default:
			return false
		}
		if name == nil || name.Kind != ast.KindIdentifier || name.Text() != r[i] {
			return false
		}
	}
	return len(r) > 0 && node != nil && node.Kind == ast.KindIdentifier && node.Text() == r[0]
}

// FindEffectModuleReference finds the reference to the Effect module from the
// file's imports. See FindModuleReference.
func FindEffectModuleReference(sf *ast.SourceFile) ModuleReference {
	return FindModuleReference(sf, "Effect")
}

// FindModuleReference finds the reference to the given module of the "effect"
// package from the file's imports. See FindModuleReferenceForPackage.
func FindModuleReference(sf *ast.SourceFile, exportName string) ModuleReference {
	return FindModuleReferenceForPackage(sf, "effect", exportName)
}

// FindModuleReferenceForPackage finds the reference to the given module of a
// package from the file's imports. It recognises:
//   - Namespace imports: import * as X from "<packageName>/<moduleName>" → X
//   - Named imports: import { <moduleName> as X } from "<packageName>" → X
//   - Namespace imports of the barrel: import * as X from "<packageName>" → X.<moduleName>
//
// The first two forms are preferred over the barrel form. Type-only imports
// are skipped, as they cannot be referenced from values. Falls back to
// moduleName if not found; no import is added.
func FindModuleReferenceForPackage(sf *ast.SourceFile, packageName string, moduleName string) ModuleReference {
	if sf == nil {
		return ModuleReference{moduleName}
	}
	var barrel ModuleReference
	for _, stmt := range sf.Statements.Nodes {
		if stmt.Kind != ast.KindImportDeclaration {
			continue
		}
		importDecl := stmt.AsImportDeclaration()
		if importDecl.ModuleSpecifier == nil || importDecl.ImportClause == nil {
			continue
		}
		specifier := scanner.GetTextOfNode(importDecl.ModuleSpecifier)
		if len(specifier) >= 2 && (specifier[0] == '"' || specifier[0] == '\'') {
			specifier = specifier[1 : len(specifier)-1]
		}

		clause := importDecl.ImportClause.AsImportClause()
		if clause.NamedBindings == nil {
			continue
		}

		if clause.NamedBindings.Kind == ast.KindNamespaceImport {
			nsImport := clause.NamedBindings.AsNamespaceImport()
			if nsImport.Name() == nil || ast.IsPartOfTypeOnlyImportOrExportDeclaration(clause.NamedBindings) {
				continue
			}
			name := scanner.GetTextOfNode(nsImport.Name())
			// import * as X from "<packageName>/<moduleName>"
			if specifier == packageName+"/"+moduleName {
				return ModuleReference{name}
			}
			// import * as X from "<packageName>"
			if specifier == packageName && barrel == nil {
				barrel = ModuleReference{name, moduleName}
			}
			continue
		}

		// import { <moduleName> as X } from "<packageName>"
		if specifier == packageName && clause.NamedBindings.Kind == ast.KindNamedImports {
			namedImports := clause.NamedBindings.AsNamedImports()
			if namedImports.Elements == nil {
				continue
			}
			for _, elem := range namedImports.Elements.Nodes {
				if ast.IsPartOfTypeOnlyImportOrExportDeclaration(elem) {
					continue
				}
				spec := elem.AsImportSpecifier()
				importedName := ""
				if spec.PropertyName != nil {
					importedName = scanner.GetTextOfNode(spec.PropertyName)
				} else {
					importedName = scanner.GetTextOfNode(spec.Name())
				}
				if importedName == moduleName {
					return ModuleReference{scanner.GetTextOfNode(spec.Name())}
				}
			}
		}
	}
	if barrel != nil {
		return barrel
	}
	return ModuleReference{moduleName}
}
