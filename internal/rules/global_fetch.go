package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var GlobalFetch = rule.Rule{
	Name:            "globalFetch",
	Group:           "effectNative",
	Description:     "Warns when using the global fetch function outside Effect generators instead of the Effect HTTP client",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Prefer_using_HttpClient_from_0_instead_of_the_global_fetch_function_effect_globalFetch.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runGlobalFetch(ctx, false)
	},
}

var GlobalFetchInEffect = rule.Rule{
	Name:            "globalFetchInEffect",
	Group:           "effectNative",
	Description:     "Warns when using the global fetch function inside Effect generators instead of the Effect HTTP client",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Prefer_using_HttpClient_from_0_instead_of_the_global_fetch_function_inside_Effect_generators_effect_globalFetchInEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runGlobalFetch(ctx, true)
	},
}

func runGlobalFetch(ctx *rule.Context, checkInEffect bool) []*ast.Diagnostic {
	fetchSymbol := ctx.Checker.ResolveName("fetch", nil, ast.SymbolFlagsValue, false)
	if fetchSymbol == nil {
		return nil
	}

	packageName := "effect/unstable/http"
	if ctx.TypeParser.SupportedEffectVersion() == typeparser.EffectMajorV3 {
		packageName = "@effect/platform"
	}

	message := tsdiag.Prefer_using_HttpClient_from_0_instead_of_the_global_fetch_function_effect_globalFetch
	if checkInEffect {
		message = tsdiag.Prefer_using_HttpClient_from_0_instead_of_the_global_fetch_function_inside_Effect_generators_effect_globalFetchInEffect
	}

	var diags []*ast.Diagnostic
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindCallExpression {
			inEffect := ctx.TypeParser.GetEffectContextFlags(node)&typeparser.EffectContextFlagCanYieldEffect != 0
			if inEffect == checkInEffect {
				call := node.AsCallExpression()
				if ctx.TypeParser.ResolveToGlobalSymbol(ctx.Checker.GetSymbolAtLocation(call.Expression)) == fetchSymbol {
					diags = append(diags, ctx.NewDiagnostic(
						ctx.SourceFile,
						scanner.GetErrorRangeForNode(ctx.SourceFile, call.Expression),
						message,
						nil,
						packageName,
					))
				}
			}
		}

		node.ForEachChild(walk)
		return false
	}

	walk(ctx.SourceFile.AsNode())

	return diags
}
