package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var AbortControllerInEffect = rule.Rule{
	Name:            "abortControllerInEffect",
	Group:           "effectNative",
	Description:     "Warns when manually constructing AbortController inside Effect generators instead of using Effect.abortSignal",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.AbortController_is_manually_constructed_inside_Effect_code_Use_Effect_abortSignal_for_Effect_managed_cancellation_effect_abortControllerInEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		abortControllerSymbol := ctx.Checker.ResolveName("AbortController", nil, ast.SymbolFlagsValue, false)
		if abortControllerSymbol == nil {
			return nil
		}

		var diags []*ast.Diagnostic
		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if node == nil {
				return false
			}

			if node.Kind == ast.KindNewExpression && ctx.TypeParser.GetEffectContextFlags(node)&typeparser.EffectContextFlagCanYieldEffect != 0 {
				newExpr := node.AsNewExpression()
				if ctx.TypeParser.ResolveToGlobalSymbol(ctx.TypeParser.GetSymbolAtLocation(newExpr.Expression)) == abortControllerSymbol {
					diags = append(diags, ctx.NewDiagnostic(
						ctx.SourceFile,
						scanner.GetErrorRangeForNode(ctx.SourceFile, node),
						tsdiag.AbortController_is_manually_constructed_inside_Effect_code_Use_Effect_abortSignal_for_Effect_managed_cancellation_effect_abortControllerInEffect,
						nil,
					))
				}
			}

			node.ForEachChild(walk)
			return false
		}

		walk(ctx.SourceFile.AsNode())
		return diags
	},
}
