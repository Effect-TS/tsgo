package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var GlobalDate = rule.Rule{
	Name:            "globalDate",
	Group:           "effectNative",
	Description:     "Warns when using Date.now() or new Date() outside Effect generators instead of Clock/DateTime",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Prefer_using_Clock_or_DateTime_from_Effect_instead_of_Date_now_effect_globalDate.Code(),
		tsdiag.Prefer_using_DateTime_from_Effect_instead_of_new_Date_effect_globalDate.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runGlobalDate(ctx, false)
	},
}

var GlobalDateInEffect = rule.Rule{
	Name:            "globalDateInEffect",
	Group:           "effectNative",
	Description:     "Warns when using Date.now() or new Date() inside Effect generators instead of Clock/DateTime",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Prefer_using_Clock_or_DateTime_from_Effect_instead_of_Date_now_inside_Effect_generators_effect_globalDateInEffect.Code(),
		tsdiag.Prefer_using_DateTime_from_Effect_instead_of_new_Date_inside_Effect_generators_effect_globalDateInEffect.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runGlobalDate(ctx, true)
	},
}

func runGlobalDate(ctx *rule.Context, checkInEffect bool) []*ast.Diagnostic {
	dateSymbol := ctx.Checker.ResolveName("Date", nil, ast.SymbolFlagsValue, false)
	if dateSymbol == nil {
		return nil
	}

	var diags []*ast.Diagnostic
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		inEffect := ctx.TypeParser.GetEffectContextFlags(node)&typeparser.EffectContextFlagCanYieldEffect != 0
		if inEffect == checkInEffect {
			var objectNode *ast.Node
			message := tsdiag.Prefer_using_Clock_or_DateTime_from_Effect_instead_of_Date_now_effect_globalDate

			switch node.Kind {
			case ast.KindCallExpression:
				call := node.AsCallExpression()
				if call.Expression.Kind == ast.KindPropertyAccessExpression {
					prop := call.Expression.AsPropertyAccessExpression()
					if prop.Name().Text() == "now" {
						objectNode = prop.Expression
						if checkInEffect {
							message = tsdiag.Prefer_using_Clock_or_DateTime_from_Effect_instead_of_Date_now_inside_Effect_generators_effect_globalDateInEffect
						}
					}
				}
			case ast.KindNewExpression:
				objectNode = node.AsNewExpression().Expression
				message = tsdiag.Prefer_using_DateTime_from_Effect_instead_of_new_Date_effect_globalDate
				if checkInEffect {
					message = tsdiag.Prefer_using_DateTime_from_Effect_instead_of_new_Date_inside_Effect_generators_effect_globalDateInEffect
				}
			}

			if objectNode != nil && ctx.TypeParser.ResolveToGlobalSymbol(ctx.Checker.GetSymbolAtLocation(objectNode)) == dateSymbol {
				diags = append(diags, ctx.NewDiagnostic(
					ctx.SourceFile,
					scanner.GetErrorRangeForNode(ctx.SourceFile, node),
					message,
					nil,
				))
			}
		}

		node.ForEachChild(walk)
		return false
	}

	walk(ctx.SourceFile.AsNode())

	return diags
}
