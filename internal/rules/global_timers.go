package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

type globalTimerAlternative struct {
	Name        string
	Alternative string
}

var globalTimerAlternatives = map[string]globalTimerAlternative{
	"setTimeout": {
		Name:        "setTimeout",
		Alternative: "Effect.sleep or Schedule",
	},
	"setInterval": {
		Name:        "setInterval",
		Alternative: "Schedule or Effect.repeat",
	},
}

var GlobalTimers = rule.Rule{
	Name:            "globalTimers",
	Group:           "effectNative",
	Description:     "Warns when using setTimeout/setInterval outside Effect generators instead of Effect.sleep/Schedule",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Prefer_using_0_from_Effect_instead_of_1_effect_globalTimers.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runGlobalTimers(ctx, false)
	},
}

var GlobalTimersInEffect = rule.Rule{
	Name:            "globalTimersInEffect",
	Group:           "effectNative",
	Description:     "Warns when using setTimeout/setInterval inside Effect generators instead of Effect.sleep/Schedule",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Prefer_using_0_from_Effect_instead_of_1_inside_Effect_generators_effect_globalTimersInEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runGlobalTimers(ctx, true)
	},
}

func runGlobalTimers(ctx *rule.Context, checkInEffect bool) []*ast.Diagnostic {
	globalSymbols := make(map[string]*ast.Symbol, len(globalTimerAlternatives))
	for name := range globalTimerAlternatives {
		if symbol := ctx.Checker.ResolveName(name, nil, ast.SymbolFlagsValue, false); symbol != nil {
			globalSymbols[name] = symbol
		}
	}
	if len(globalSymbols) == 0 {
		return nil
	}

	message := tsdiag.Prefer_using_0_from_Effect_instead_of_1_effect_globalTimers
	if checkInEffect {
		message = tsdiag.Prefer_using_0_from_Effect_instead_of_1_inside_Effect_generators_effect_globalTimersInEffect
	}

	var diags []*ast.Diagnostic
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindCallExpression {
			inEffect := typeparser.GetEffectContextFlags(ctx.Checker, node)&typeparser.EffectContextFlagCanYieldEffect != 0
			if inEffect == checkInEffect {
				resolved := typeparser.ResolveToGlobalSymbol(ctx.Checker, ctx.Checker.GetSymbolAtLocation(node.AsCallExpression().Expression))
				if resolved != nil {
					for name, globalSymbol := range globalSymbols {
						if resolved != globalSymbol {
							continue
						}
						alt := globalTimerAlternatives[name]
						diags = append(diags, ctx.NewDiagnostic(
							ctx.SourceFile,
							scanner.GetErrorRangeForNode(ctx.SourceFile, node),
							message,
							nil,
							alt.Alternative,
							alt.Name,
						))
						break
					}
				}
			}
		}

		node.ForEachChild(walk)
		return false
	}

	walk(ctx.SourceFile.AsNode())

	return diags
}
