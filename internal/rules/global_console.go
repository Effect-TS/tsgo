package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var globalConsoleMethodAlternatives = map[string]string{
	"log":   "Effect.log or Logger",
	"warn":  "Effect.logWarning or Logger",
	"error": "Effect.logError or Logger",
	"info":  "Effect.logInfo or Logger",
	"debug": "Effect.logDebug or Logger",
	"trace": "Effect.logTrace or Logger",
}

var GlobalConsole = rule.Rule{
	Name:            "globalConsole",
	Group:           "effectNative",
	Description:     "Warns when using console methods outside Effect generators instead of Effect.log/Logger",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_code_uses_console_1_the_corresponding_Effect_logging_API_is_0_effect_globalConsole.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runGlobalConsole(ctx, false)
	},
}

var GlobalConsoleInEffect = rule.Rule{
	Name:            "globalConsoleInEffect",
	Group:           "effectNative",
	Description:     "Warns when using console methods inside Effect generators instead of Effect.log/Logger",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_Effect_code_uses_console_1_logging_in_Effect_code_is_represented_through_0_effect_globalConsoleInEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runGlobalConsole(ctx, true)
	},
}

func runGlobalConsole(ctx *rule.Context, checkInEffect bool) []*ast.Diagnostic {
	consoleSymbol := ctx.Checker.ResolveName("console", nil, ast.SymbolFlagsValue, false)
	if consoleSymbol == nil {
		return nil
	}

	message := tsdiag.This_code_uses_console_1_the_corresponding_Effect_logging_API_is_0_effect_globalConsole
	if checkInEffect {
		message = tsdiag.This_Effect_code_uses_console_1_logging_in_Effect_code_is_represented_through_0_effect_globalConsoleInEffect
	}

	var diags []*ast.Diagnostic
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindCallExpression && node.AsCallExpression().Expression.Kind == ast.KindPropertyAccessExpression {
			inEffect := ctx.TypeParser.GetEffectContextFlags(node)&typeparser.EffectContextFlagInEffect != 0
			if inEffect == checkInEffect {
				prop := node.AsCallExpression().Expression.AsPropertyAccessExpression()
				method := prop.Name().Text()
				alternative := globalConsoleMethodAlternatives[method]
				if alternative != "" && ctx.TypeParser.ResolveToGlobalSymbol(ctx.Checker.GetSymbolAtLocation(prop.Expression)) == consoleSymbol {
					diags = append(diags, ctx.NewDiagnostic(
						ctx.SourceFile,
						scanner.GetErrorRangeForNode(ctx.SourceFile, node),
						message,
						nil,
						alternative,
						method,
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
