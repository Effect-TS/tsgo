package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var GlobalFetch = rule.Rule{
	Name:            "globalFetch",
	Group:           "effectNative",
	Description:     "Warns when using the global fetch function instead of the Effect HTTP client",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Prefer_using_HttpClient_from_0_instead_of_the_global_fetch_function_effect_globalFetch.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeGlobalFetch(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(
				m.SourceFile,
				m.Location,
				tsdiag.Prefer_using_HttpClient_from_0_instead_of_the_global_fetch_function_effect_globalFetch,
				nil,
				m.PackageName,
			)
		}
		return diags
	},
}

type GlobalFetchMatch struct {
	SourceFile  *ast.SourceFile
	Location    core.TextRange
	PackageName string
}

func AnalyzeGlobalFetch(c *checker.Checker, sf *ast.SourceFile) []GlobalFetchMatch {
	fetchSymbol := c.ResolveName("fetch", nil, ast.SymbolFlagsValue, false)
	if fetchSymbol == nil {
		return nil
	}

	packageName := "effect/unstable/http"
	if typeparser.SupportedEffectVersion(c) == typeparser.EffectMajorV3 {
		packageName = "@effect/platform"
	}

	var matches []GlobalFetchMatch

	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		if node.Kind == ast.KindCallExpression {
			call := node.AsCallExpression()
			symbol := c.GetSymbolAtLocation(call.Expression)
			resolvedSymbol := symbol
			if resolvedSymbol != nil && resolvedSymbol.Flags&ast.SymbolFlagsAlias != 0 {
				resolvedSymbol = c.GetAliasedSymbol(resolvedSymbol)
			}

			if resolvedSymbol == fetchSymbol {
				matches = append(matches, GlobalFetchMatch{
					SourceFile:  sf,
					Location:    scanner.GetErrorRangeForNode(sf, call.Expression),
					PackageName: packageName,
				})
			}
		}

		node.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())

	return matches
}
