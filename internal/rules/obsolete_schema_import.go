package rules

import (
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var ObsoleteSchemaImport = rule.Rule{
	Name:            "obsoleteSchemaImport",
	Group:           "correctness",
	Description:     "Warns when importing @effect/schema in projects targeting Effect v4, where Schema is included directly in the effect package",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.This_module_reference_imports_0_which_is_obsolete_in_Effect_v4_In_Effect_v4_Schema_is_provided_directly_by_Schema_from_effect_or_effect_SlashSchema_effect_obsoleteSchemaImport.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeObsoleteSchemaImport(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(
				m.SourceFile,
				m.Location,
				tsdiag.This_module_reference_imports_0_which_is_obsolete_in_Effect_v4_In_Effect_v4_Schema_is_provided_directly_by_Schema_from_effect_or_effect_SlashSchema_effect_obsoleteSchemaImport,
				nil,
				m.ImportedSpecifier,
			)
		}
		return diags
	},
}

type ObsoleteSchemaImportMatch struct {
	SourceFile        *ast.SourceFile
	Location          core.TextRange
	ImportedSpecifier string
}

func AnalyzeObsoleteSchemaImport(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []ObsoleteSchemaImportMatch {
	if tp == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []ObsoleteSchemaImportMatch

	isObsoleteSchemaSpecifier := func(specifier string) bool {
		return specifier == "@effect/schema" || strings.HasPrefix(specifier, "@effect/schema/")
	}

	for _, stmt := range sf.Statements.Nodes {
		switch stmt.Kind {
		case ast.KindImportDeclaration:
			importDecl := stmt.AsImportDeclaration()
			if importDecl.ModuleSpecifier == nil || importDecl.ModuleSpecifier.Kind != ast.KindStringLiteral {
				continue
			}
			specifier := importDecl.ModuleSpecifier.AsStringLiteral().Text
			if isObsoleteSchemaSpecifier(specifier) {
				matches = append(matches, ObsoleteSchemaImportMatch{
					SourceFile:        sf,
					Location:          scanner.GetErrorRangeForNode(sf, importDecl.ModuleSpecifier),
					ImportedSpecifier: specifier,
				})
			}

		case ast.KindExportDeclaration:
			exportDecl := stmt.AsExportDeclaration()
			if exportDecl.ModuleSpecifier == nil || exportDecl.ModuleSpecifier.Kind != ast.KindStringLiteral {
				continue
			}
			specifier := exportDecl.ModuleSpecifier.AsStringLiteral().Text
			if isObsoleteSchemaSpecifier(specifier) {
				matches = append(matches, ObsoleteSchemaImportMatch{
					SourceFile:        sf,
					Location:          scanner.GetErrorRangeForNode(sf, exportDecl.ModuleSpecifier),
					ImportedSpecifier: specifier,
				})
			}

		case ast.KindVariableStatement:
			varStmt := stmt.AsVariableStatement()
			if varStmt.DeclarationList == nil {
				continue
			}
			declList := varStmt.DeclarationList.AsVariableDeclarationList()
			if declList.Declarations == nil {
				continue
			}
			for _, declNode := range declList.Declarations.Nodes {
				decl := declNode.AsVariableDeclaration()
				if decl.Initializer == nil || decl.Initializer.Kind != ast.KindCallExpression {
					continue
				}
				callExpr := decl.Initializer.AsCallExpression()
				if callExpr.Expression == nil || callExpr.Expression.Kind != ast.KindIdentifier {
					continue
				}
				if scanner.GetTextOfNode(callExpr.Expression) != "require" {
					continue
				}
				if callExpr.Arguments == nil || len(callExpr.Arguments.Nodes) != 1 {
					continue
				}
				arg := callExpr.Arguments.Nodes[0]
				if arg.Kind != ast.KindStringLiteral {
					continue
				}
				specifier := arg.AsStringLiteral().Text
				if isObsoleteSchemaSpecifier(specifier) {
					matches = append(matches, ObsoleteSchemaImportMatch{
						SourceFile:        sf,
						Location:          scanner.GetErrorRangeForNode(sf, arg),
						ImportedSpecifier: specifier,
					})
				}
			}
		}
	}

	return matches
}
