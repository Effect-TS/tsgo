package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var GlobalRandom = rule.Rule{
	Name:            "globalRandom",
	Group:           "effectNative",
	Description:     "Warns when using Math.random() outside Effect generators instead of the Random service",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_code_uses_Math_random_randomness_is_represented_through_the_Effect_Random_service_effect_globalRandom.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runGlobalRandom(ctx, false)
	},
}

var GlobalRandomInEffect = rule.Rule{
	Name:            "globalRandomInEffect",
	Group:           "effectNative",
	Description:     "Warns when using Math.random() inside Effect generators instead of the Random service",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_Effect_code_uses_Math_random_randomness_is_represented_through_the_Effect_Random_service_effect_globalRandomInEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runGlobalRandom(ctx, true)
	},
}

func runGlobalRandom(ctx *rule.Context, checkInEffect bool) []*ast.Diagnostic {
	mathSymbol := ctx.Checker.ResolveName("Math", nil, ast.SymbolFlagsValue, false)
	if mathSymbol == nil {
		return nil
	}

	message := tsdiag.This_code_uses_Math_random_randomness_is_represented_through_the_Effect_Random_service_effect_globalRandom
	if checkInEffect {
		message = tsdiag.This_Effect_code_uses_Math_random_randomness_is_represented_through_the_Effect_Random_service_effect_globalRandomInEffect
	}

	var diags []*ast.Diagnostic
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindCallExpression && node.AsCallExpression().Expression.Kind == ast.KindPropertyAccessExpression {
			inEffect := ctx.TypeParser.GetEffectContextFlags(node)&typeparser.EffectContextFlagCanYieldEffect != 0
			if inEffect == checkInEffect {
				prop := node.AsCallExpression().Expression.AsPropertyAccessExpression()
				if prop.Name().Text() == "random" && ctx.TypeParser.ResolveToGlobalSymbol(ctx.Checker.GetSymbolAtLocation(prop.Expression)) == mathSymbol {
					diags = append(diags, ctx.NewDiagnostic(
						ctx.SourceFile,
						scanner.GetErrorRangeForNode(ctx.SourceFile, node),
						message,
						nil,
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
