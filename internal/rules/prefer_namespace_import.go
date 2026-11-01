package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/importstyle"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
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
	matches := AnalyzePreferNamespaceImport(ctx.TypeParser, ctx.Checker, ctx.SourceFile, ctx.Options)
	diags := make([]*ast.Diagnostic, len(matches))
	for i, match := range matches {
		if match.Target.Specifier == "" {
			diags[i] = ctx.NewDiagnostic(
				ctx.SourceFile,
				match.Location,
				tsdiag.X_0_is_imported_by_name_from_1_but_namespaceImportPackages_expects_a_namespace_import_of_the_module_that_declares_0_effect_preferNamespaceImport,
				nil,
				match.ImportedName,
				match.Specifier,
			)
			continue
		}
		diags[i] = ctx.NewDiagnostic(
			ctx.SourceFile,
			match.Location,
			tsdiag.X_0_is_imported_by_name_from_1_but_namespaceImportPackages_expects_a_namespace_import_Use_2_instead_effect_preferNamespaceImport,
			nil,
			match.ImportedName,
			match.Specifier,
			namespaceImportText(match.Target, match.TypeOnly),
		)
	}
	return diags
}

// PreferNamespaceImportMatch is a named import that namespaceImportPackages
// expects as a namespace import.
type PreferNamespaceImportMatch struct {
	ImportDeclaration *ast.Node
	// Element is the reported import specifier.
	Element  *ast.Node
	Location core.TextRange
	// ImportedName is the name the import specifier imports from Specifier.
	ImportedName string
	// Specifier is the module specifier the declaration imports.
	Specifier string
	// Target is the namespace import the element should become. Its Specifier
	// is empty when the declaring module could not be derived.
	Target   NamespaceImportTarget
	TypeOnly bool
}

// AnalyzePreferNamespaceImport finds the import specifiers that preferNamespaceImport reports.
func AnalyzePreferNamespaceImport(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, options *etscore.ResolvedEffectPluginOptions) []PreferNamespaceImportMatch {
	if options == nil || len(options.NamespaceImportPackages) == 0 {
		return nil
	}
	policy := importstyle.NewPolicy(options)
	followReexports := options.TopLevelNamedReexports == etscore.TopLevelNamedReexportsFollow

	var matches []PreferNamespaceImportMatch
	for _, stmt := range sf.Statements.Nodes {
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

		moduleSymbol := tp.GetSymbolAtLocation(importDecl.ModuleSpecifier)
		if moduleSymbol != nil && ast.TSGetSymbolFlags(moduleSymbol)&ast.SymbolFlagsAlias != 0 {
			moduleSymbol = c.GetAliasedSymbol(moduleSymbol)
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
			exportSymbol := c.TryGetMemberInModuleExports(importedName.Text(), moduleSymbol)
			if exportSymbol == nil {
				continue
			}

			target, isReexport := namespaceImportTarget(c, exportSymbol, moduleFile, specifier, element.Name().Text(), importedName.Text())
			if isReexport && !followReexports {
				continue
			}
			matches = append(matches, PreferNamespaceImportMatch{
				ImportDeclaration: stmt,
				Element:           element,
				Location:          scanner.GetErrorRangeForNode(sf, element),
				ImportedName:      importedName.Text(),
				Specifier:         specifier,
				Target:            target,
				TypeOnly:          clause.IsTypeOnly() || element.IsTypeOnly(),
			})
		}
	}
	return matches
}

// NamespaceImportTarget is the namespace import an offending named import should become.
// An empty Specifier means the declaring module could not be derived.
type NamespaceImportTarget struct {
	// Name is the namespace binding the message names.
	Name      string
	Specifier string
	// Member is the name the import refers to inside the namespace, e.g. "map"
	// for `import { map } from "effect/Array"`. It is empty when the imported
	// binding is the namespace itself, as for `import { Effect } from "effect"`.
	Member string
}

// namespaceImportTarget classifies an export of the module at specifier.
// isReexport is true for exports that the module forwards from another module
// by name or through `export *`; those follow topLevelNamedReexports.
func namespaceImportTarget(c *checker.Checker, exportSymbol *ast.Symbol, moduleFile *ast.SourceFile, specifier string, localName string, importedName string) (NamespaceImportTarget, bool) {
	declarations := ast.TSGetSymbolDeclarations(exportSymbol)
	if ast.TSGetSymbolFlags(exportSymbol)&ast.SymbolFlagsAlias == 0 {
		for _, declaration := range declarations {
			if ast.GetSourceFileOfNode(declaration) == moduleFile {
				return NamespaceImportTarget{Name: importstyle.NamespaceName(specifier), Specifier: specifier, Member: importedName}, false
			}
		}
		return NamespaceImportTarget{}, true
	}

	if len(declarations) == 0 || ast.GetSourceFileOfNode(declarations[0]) != moduleFile {
		return NamespaceImportTarget{}, true
	}
	declaration := declarations[0]
	switch declaration.Kind {
	case ast.KindNamespaceExport:
		target := reexportTarget(declaration.Parent, specifier)
		return NamespaceImportTarget{Name: localName, Specifier: target}, false
	case ast.KindExportSpecifier:
		if exportDeclaration := declaration.Parent.Parent; exportDeclaration.AsExportDeclaration().ModuleSpecifier != nil {
			target := reexportTarget(exportDeclaration, specifier)
			member := declaration.Name()
			if propertyName := declaration.PropertyName(); propertyName != nil {
				member = propertyName
			}
			return NamespaceImportTarget{Name: importstyle.NamespaceName(target), Specifier: target, Member: member.Text()}, true
		}
	}
	// `export { local }` and `export default local` alias a declaration that
	// may live in the module itself.
	if aliased := c.GetImmediateAliasedSymbol(exportSymbol); aliased != nil && ast.TSGetSymbolFlags(aliased)&ast.SymbolFlagsAlias == 0 {
		return namespaceImportTarget(c, aliased, moduleFile, specifier, localName, importedName)
	}
	return NamespaceImportTarget{}, true
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

func namespaceImportText(target NamespaceImportTarget, typeOnly bool) string {
	keyword := "import "
	if typeOnly {
		keyword = "import type "
	}
	return keyword + "* as " + target.Name + " from \"" + target.Specifier + "\""
}
