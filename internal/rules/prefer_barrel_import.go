package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/importstyle"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
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

func runPreferBarrelImport(ctx *rule.Context) []*ast.Diagnostic {
	if ctx.Options == nil || len(ctx.Options.BarrelImportPackages) == 0 {
		return nil
	}
	barrels := ctx.TypeParser.BarrelReader()
	if barrels == nil {
		return nil
	}
	policy := importstyle.NewPolicy(ctx.Options)

	var diags []*ast.Diagnostic
	for _, stmt := range ctx.SourceFile.Statements.Nodes {
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
		resolved := ctx.Program.GetResolvedModuleFromModuleSpecifier(ctx.SourceFile, importDecl.ModuleSpecifier)
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
		keyword := "import "
		if importDecl.ImportClause.IsTypeOnly() {
			keyword = "import type "
		}
		diags = append(diags, ctx.NewDiagnostic(
			ctx.SourceFile,
			ctx.GetErrorRange(importDecl.ModuleSpecifier),
			tsdiag.X_0_is_imported_directly_but_barrelImportPackages_expects_named_imports_from_the_barrel_1_Use_2_instead_effect_preferBarrelImport,
			nil,
			specifier,
			barrel.Specifier,
			keyword+"{ "+binding+" } from \""+barrel.Specifier+"\"",
		))
	}
	return diags
}
