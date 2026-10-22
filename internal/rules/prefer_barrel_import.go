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

var PreferBarrelImport = rule.Rule{
	Name:            "preferBarrelImport",
	Group:           "style",
	Description:     "Reports imports of individual modules from packages configured in barrelImportPackages that should be named imports from the package barrel",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.X_0_is_imported_directly_but_barrelImportPackages_expects_named_imports_from_the_barrel_1_Use_2_instead_effect_preferBarrelImport.Code()},
	Run:             runPreferBarrelImport,
}

// PreferBarrelImportMatch is an import declaration that barrelImportPackages
// expects as a named import from the barrel that forwards its module.
type PreferBarrelImportMatch struct {
	ImportDeclaration *ast.Node
	Location          core.TextRange
	// Specifier is the module specifier the declaration imports.
	Specifier string
	// Barrel is the barrel that forwards the module.
	Barrel importstyle.Barrel
	// Binding is the import specifier the barrel import needs, e.g. "Option as O".
	Binding  string
	TypeOnly bool
}

// ImportText returns the import declaration the message suggests.
func (m PreferBarrelImportMatch) ImportText() string {
	keyword := "import "
	if m.TypeOnly {
		keyword = "import type "
	}
	return keyword + "{ " + m.Binding + " } from \"" + m.Barrel.Specifier + "\""
}

func runPreferBarrelImport(ctx *rule.Context) []*ast.Diagnostic {
	matches := AnalyzePreferBarrelImport(ctx.TypeParser, ctx.Program, ctx.SourceFile, ctx.Options)
	diags := make([]*ast.Diagnostic, len(matches))
	for i, match := range matches {
		diags[i] = ctx.NewDiagnostic(
			ctx.SourceFile,
			match.Location,
			tsdiag.X_0_is_imported_directly_but_barrelImportPackages_expects_named_imports_from_the_barrel_1_Use_2_instead_effect_preferBarrelImport,
			nil,
			match.Specifier,
			match.Barrel.Specifier,
			match.ImportText(),
		)
	}
	return diags
}

// AnalyzePreferBarrelImport finds the import declarations that preferBarrelImport reports.
func AnalyzePreferBarrelImport(tp *typeparser.TypeParser, program checker.Program, sf *ast.SourceFile, options *etscore.ResolvedEffectPluginOptions) []PreferBarrelImportMatch {
	if options == nil || len(options.BarrelImportPackages) == 0 {
		return nil
	}
	barrels := tp.BarrelReader()
	if barrels == nil {
		return nil
	}
	policy := importstyle.NewPolicy(options)

	var matches []PreferBarrelImportMatch
	for _, stmt := range sf.Statements.Nodes {
		if stmt.Kind != ast.KindImportDeclaration {
			continue
		}
		importDecl := stmt.AsImportDeclaration()
		if importDecl.ModuleSpecifier == nil || importDecl.ModuleSpecifier.Kind != ast.KindStringLiteral || importDecl.ImportClause == nil {
			continue
		}
		namedBindings := importDecl.ImportClause.AsImportClause().NamedBindings
		if namedBindings == nil {
			continue
		}
		specifier := importDecl.ModuleSpecifier.Text()
		if policy.Style(specifier) != importstyle.StyleBarrel {
			continue
		}
		resolved := program.GetResolvedModuleFromModuleSpecifier(sf, importDecl.ModuleSpecifier)
		if resolved == nil {
			continue
		}
		barrel, ok := barrels.Find(specifier, resolved.ResolvedFileName)
		if !ok {
			continue
		}
		binding := barrel.ExportName
		if namedBindings.Kind == ast.KindNamespaceImport {
			if localName := namedBindings.Name().Text(); localName != barrel.ExportName {
				binding = barrel.ExportName + " as " + localName
			}
		}
		matches = append(matches, PreferBarrelImportMatch{
			ImportDeclaration: stmt,
			Location:          scanner.GetErrorRangeForNode(sf, importDecl.ModuleSpecifier),
			Specifier:         specifier,
			Barrel:            barrel,
			Binding:           binding,
			TypeOnly:          importDecl.ImportClause.IsTypeOnly(),
		})
	}
	return matches
}
