package effectutil

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// FindEffectModuleIdentifier finds the local name for the Effect module from imports.
// It checks both namespace imports (import * as X from "effect/Effect") and
// named imports (import { Effect } from "effect"), falling back to "Effect".
func FindEffectModuleIdentifier(sf *ast.SourceFile) string {
	if sf == nil {
		return "Effect"
	}
	for _, stmt := range sf.Statements.Nodes {
		if stmt.Kind != ast.KindImportDeclaration {
			continue
		}
		importDecl := stmt.AsImportDeclaration()
		if importDecl.ModuleSpecifier == nil || importDecl.ImportClause == nil {
			continue
		}
		moduleName := scanner.GetTextOfNode(importDecl.ModuleSpecifier)
		if len(moduleName) >= 2 && (moduleName[0] == '"' || moduleName[0] == '\'') {
			moduleName = moduleName[1 : len(moduleName)-1]
		}

		clause := importDecl.ImportClause.AsImportClause()
		if clause.NamedBindings == nil {
			continue
		}

		// Check namespace import: import * as X from "effect/Effect"
		if moduleName == "effect/Effect" && clause.NamedBindings.Kind == ast.KindNamespaceImport {
			nsImport := clause.NamedBindings.AsNamespaceImport()
			if nsImport.Name() != nil {
				return scanner.GetTextOfNode(nsImport.Name())
			}
		}

		// Check named imports: import { Effect } from "effect"
		if moduleName == "effect" && clause.NamedBindings.Kind == ast.KindNamedImports {
			namedImports := clause.NamedBindings.AsNamedImports()
			if namedImports.Elements == nil {
				continue
			}
			for _, elem := range namedImports.Elements.Nodes {
				spec := elem.AsImportSpecifier()
				importedName := ""
				if spec.PropertyName != nil {
					importedName = scanner.GetTextOfNode(spec.PropertyName)
				} else {
					importedName = scanner.GetTextOfNode(spec.Name())
				}
				if importedName == "Effect" {
					return scanner.GetTextOfNode(spec.Name())
				}
			}
		}
	}
	return "Effect"
}

// FindModuleIdentifier resolves the imported identifier name for the given export
// from the "effect" package. Falls back to the provided exportName if not found.
// It checks named imports: import { ExportName as Alias } from "effect".
func FindModuleIdentifier(sf *ast.SourceFile, exportName string) string {
	if sf == nil {
		return exportName
	}
	for _, stmt := range sf.Statements.Nodes {
		if stmt.Kind != ast.KindImportDeclaration {
			continue
		}
		importDecl := stmt.AsImportDeclaration()
		if importDecl.ModuleSpecifier == nil {
			continue
		}
		moduleName := scanner.GetTextOfNode(importDecl.ModuleSpecifier)
		if len(moduleName) >= 2 && (moduleName[0] == '"' || moduleName[0] == '\'') {
			moduleName = moduleName[1 : len(moduleName)-1]
		}

		if importDecl.ImportClause == nil {
			continue
		}
		clause := importDecl.ImportClause.AsImportClause()
		if clause.NamedBindings == nil {
			continue
		}

		// Check namespace import: import * as X from "effect/<exportName>"
		if moduleName == "effect/"+exportName && clause.NamedBindings.Kind == ast.KindNamespaceImport {
			nsImport := clause.NamedBindings.AsNamespaceImport()
			if nsImport.Name() != nil {
				return scanner.GetTextOfNode(nsImport.Name())
			}
		}

		// Check named imports: import { ExportName as Alias } from "effect"
		if moduleName == "effect" && clause.NamedBindings.Kind == ast.KindNamedImports {
			namedImports := clause.NamedBindings.AsNamedImports()
			if namedImports.Elements == nil {
				continue
			}
			for _, elem := range namedImports.Elements.Nodes {
				spec := elem.AsImportSpecifier()
				importedName := ""
				if spec.PropertyName != nil {
					importedName = scanner.GetTextOfNode(spec.PropertyName)
				} else {
					importedName = scanner.GetTextOfNode(spec.Name())
				}
				if importedName == exportName {
					return scanner.GetTextOfNode(spec.Name())
				}
			}
		}
	}
	return exportName
}

// FindModuleIdentifierForPackage resolves the imported identifier name for the given
// module from an arbitrary package. It checks:
//   - Namespace imports: import * as X from "<packageName>/<moduleName>"
//   - Named imports: import { <moduleName> as X } from "<packageName>"
//
// Falls back to moduleName if not found.
func FindModuleIdentifierForPackage(sf *ast.SourceFile, packageName string, moduleName string) string {
	if sf == nil {
		return moduleName
	}
	for _, stmt := range sf.Statements.Nodes {
		if stmt.Kind != ast.KindImportDeclaration {
			continue
		}
		importDecl := stmt.AsImportDeclaration()
		if importDecl.ModuleSpecifier == nil {
			continue
		}
		specifier := scanner.GetTextOfNode(importDecl.ModuleSpecifier)
		if len(specifier) >= 2 && (specifier[0] == '"' || specifier[0] == '\'') {
			specifier = specifier[1 : len(specifier)-1]
		}

		if importDecl.ImportClause == nil {
			continue
		}
		clause := importDecl.ImportClause.AsImportClause()
		if clause.NamedBindings == nil {
			continue
		}

		// Check namespace import: import * as X from "<packageName>/<moduleName>"
		if specifier == packageName+"/"+moduleName && clause.NamedBindings.Kind == ast.KindNamespaceImport {
			nsImport := clause.NamedBindings.AsNamespaceImport()
			if nsImport.Name() != nil {
				return scanner.GetTextOfNode(nsImport.Name())
			}
		}

		// Check named imports: import { <moduleName> as X } from "<packageName>"
		if specifier == packageName && clause.NamedBindings.Kind == ast.KindNamedImports {
			namedImports := clause.NamedBindings.AsNamedImports()
			if namedImports.Elements == nil {
				continue
			}
			for _, elem := range namedImports.Elements.Nodes {
				spec := elem.AsImportSpecifier()
				importedName := ""
				if spec.PropertyName != nil {
					importedName = scanner.GetTextOfNode(spec.PropertyName)
				} else {
					importedName = scanner.GetTextOfNode(spec.Name())
				}
				if importedName == moduleName {
					return scanner.GetTextOfNode(spec.Name())
				}
			}
		}
	}
	return moduleName
}
