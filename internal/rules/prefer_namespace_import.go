package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/importstyle"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

var PreferNamespaceImport = rule.Rule{
	Name:            "preferNamespaceImport",
	Group:           "style",
	Description:     "Reports named imports from packages configured in namespaceImportPackages that should be namespace imports",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.X_0_is_imported_by_name_from_1_but_namespaceImportPackages_expects_a_namespace_import_Use_2_instead_effect_preferNamespaceImport.Code(),
		tsdiag.X_0_is_imported_by_name_from_1_but_namespaceImportPackages_expects_a_namespace_import_of_the_module_that_declares_0_effect_preferNamespaceImport.Code(),
	},
	Run: runPreferNamespaceImport,
}

func runPreferNamespaceImport(ctx *rule.Context) []*ast.Diagnostic {
	if ctx.Options == nil || len(ctx.Options.NamespaceImportPackages) == 0 {
		return nil
	}
	policy := importstyle.NewPolicy(ctx.Options)
	followReexports := ctx.Options.TopLevelNamedReexports == etscore.TopLevelNamedReexportsFollow

	var diags []*ast.Diagnostic
	for _, stmt := range ctx.SourceFile.Statements.Nodes {
		if stmt.Kind != ast.KindImportDeclaration {
			continue
		}
		importDecl := stmt.AsImportDeclaration()
		if importDecl.ModuleSpecifier == nil || importDecl.ModuleSpecifier.Kind != ast.KindStringLiteral {
			continue
		}
		clause := importDecl.ImportClause
		if clause == nil {
			continue
		}
		namedBindings := clause.AsImportClause().NamedBindings
		if namedBindings == nil || namedBindings.Kind != ast.KindNamedImports {
			continue
		}
		specifier := importDecl.ModuleSpecifier.Text()
		if policy.Style(specifier) != importstyle.StyleNamespace {
			continue
		}

		moduleSymbol := ctx.TypeParser.GetSymbolAtLocation(importDecl.ModuleSpecifier)
		if moduleSymbol != nil && ast.TSGetSymbolFlags(moduleSymbol)&ast.SymbolFlagsAlias != 0 {
			moduleSymbol = ctx.Checker.GetAliasedSymbol(moduleSymbol)
		}
		if moduleSymbol == nil {
			continue
		}
		moduleFile := symbolSourceFile(moduleSymbol)

		elements := namedBindings.AsNamedImports().Elements
		if elements == nil {
			continue
		}
		for _, element := range elements.Nodes {
			importSpecifier := element.AsImportSpecifier()
			importedName := element.Name()
			if importSpecifier.PropertyName != nil {
				importedName = importSpecifier.PropertyName
			}
			if importedName.Kind != ast.KindIdentifier {
				continue
			}
			exportSymbol := ctx.Checker.TryGetMemberInModuleExports(importedName.Text(), moduleSymbol)
			if exportSymbol == nil {
				continue
			}

			target, isReexport := namespaceImportTarget(ctx.Checker, exportSymbol, moduleFile, specifier, element.Name().Text())
			if isReexport && !followReexports {
				continue
			}
			typeOnly := clause.IsTypeOnly() || element.IsTypeOnly()
			location := ctx.GetErrorRange(element)
			if target.specifier == "" {
				diags = append(diags, ctx.NewDiagnostic(
					ctx.SourceFile,
					location,
					tsdiag.X_0_is_imported_by_name_from_1_but_namespaceImportPackages_expects_a_namespace_import_of_the_module_that_declares_0_effect_preferNamespaceImport,
					nil,
					importedName.Text(),
					specifier,
				))
				continue
			}
			diags = append(diags, ctx.NewDiagnostic(
				ctx.SourceFile,
				location,
				tsdiag.X_0_is_imported_by_name_from_1_but_namespaceImportPackages_expects_a_namespace_import_Use_2_instead_effect_preferNamespaceImport,
				nil,
				importedName.Text(),
				specifier,
				namespaceImportText(target, typeOnly),
			))
		}
	}
	return diags
}

// namespaceImport is the namespace import an offending named import should become.
// An empty specifier means the declaring module could not be derived.
type namespaceImport struct {
	name      string
	specifier string
}

// namespaceImportTarget classifies an export of the module at specifier.
// isReexport is true for exports that the module forwards from another module
// by name or through `export *`; those follow topLevelNamedReexports.
func namespaceImportTarget(c *checker.Checker, exportSymbol *ast.Symbol, moduleFile *ast.SourceFile, specifier string, localName string) (namespaceImport, bool) {
	declarations := ast.TSGetSymbolDeclarations(exportSymbol)
	if ast.TSGetSymbolFlags(exportSymbol)&ast.SymbolFlagsAlias == 0 {
		for _, declaration := range declarations {
			if ast.GetSourceFileOfNode(declaration) == moduleFile {
				return namespaceImport{name: importstyle.NamespaceName(specifier), specifier: specifier}, false
			}
		}
		return namespaceImport{}, true
	}

	if len(declarations) == 0 || ast.GetSourceFileOfNode(declarations[0]) != moduleFile {
		return namespaceImport{}, true
	}
	declaration := declarations[0]
	switch declaration.Kind {
	case ast.KindNamespaceExport:
		target := reexportTarget(declaration.Parent, specifier)
		return namespaceImport{name: localName, specifier: target}, false
	case ast.KindExportSpecifier:
		if exportDeclaration := declaration.Parent.Parent; exportDeclaration.AsExportDeclaration().ModuleSpecifier != nil {
			target := reexportTarget(exportDeclaration, specifier)
			return namespaceImport{name: importstyle.NamespaceName(target), specifier: target}, true
		}
	}
	// `export { local }` and `export default local` alias a declaration that
	// may live in the module itself.
	if aliased := c.GetImmediateAliasedSymbol(exportSymbol); aliased != nil && ast.TSGetSymbolFlags(aliased)&ast.SymbolFlagsAlias == 0 {
		return namespaceImportTarget(c, aliased, moduleFile, specifier, localName)
	}
	return namespaceImport{}, true
}

// reexportTarget derives the public specifier of the module an export
// declaration in the barrel at specifier forwards from.
func reexportTarget(exportDeclaration *ast.Node, specifier string) string {
	if exportDeclaration == nil || exportDeclaration.Kind != ast.KindExportDeclaration || !importstyle.IsBarrelFile(ast.GetSourceFileOfNode(exportDeclaration).FileName()) {
		return ""
	}
	moduleSpecifier := exportDeclaration.AsExportDeclaration().ModuleSpecifier
	if moduleSpecifier == nil || moduleSpecifier.Kind != ast.KindStringLiteral {
		return ""
	}
	return importstyle.ReexportSpecifier(specifier, moduleSpecifier.Text())
}

func symbolSourceFile(symbol *ast.Symbol) *ast.SourceFile {
	for _, declaration := range ast.TSGetSymbolDeclarations(symbol) {
		if declaration.Kind == ast.KindSourceFile {
			return declaration.AsSourceFile()
		}
	}
	if declaration := ast.TSGetSymbolValueDeclaration(symbol); declaration != nil {
		return ast.GetSourceFileOfNode(declaration)
	}
	return nil
}

func namespaceImportText(target namespaceImport, typeOnly bool) string {
	keyword := "import "
	if typeOnly {
		keyword = "import type "
	}
	return keyword + "* as " + target.name + " from \"" + target.specifier + "\""
}
