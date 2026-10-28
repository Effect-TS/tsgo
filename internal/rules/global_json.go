package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

var GlobalJson = rule.Rule{
	Name:            "globalJson",
	Group:           "effectNative",
	Description:     "Warns when using JSON.parse or JSON.stringify outside Effect generators instead of Effect Schema",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_code_uses_JSON_0_which_throws_instead_of_failing_with_a_typed_error_1_effect_globalJson.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runGlobalJson(ctx, false, tsdiag.This_code_uses_JSON_0_which_throws_instead_of_failing_with_a_typed_error_1_effect_globalJson)
	},
}

var GlobalJsonInEffect = rule.Rule{
	Name:            "globalJsonInEffect",
	Group:           "effectNative",
	Description:     "Warns when using JSON.parse or JSON.stringify inside Effect generators instead of Effect Schema",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_Effect_code_uses_JSON_0_which_throws_instead_of_failing_with_a_typed_error_1_effect_globalJsonInEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runGlobalJson(ctx, true, tsdiag.This_Effect_code_uses_JSON_0_which_throws_instead_of_failing_with_a_typed_error_1_effect_globalJsonInEffect)
	},
}

func runGlobalJson(ctx *rule.Context, checkInEffect bool, message *tsdiag.Message) []*ast.Diagnostic {
	recommendation := globalJsonRecommendation(ctx.TypeParser.SupportedEffectVersion() == typeparser.EffectMajorV4)
	var diags []*ast.Diagnostic
	for _, call := range findGlobalJsonCalls(ctx, checkInEffect) {
		diags = append(diags, ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(call.node), message, nil, call.method, recommendation))
	}
	return diags
}

type globalJsonCall struct {
	node   *ast.Node
	method string
}

// findGlobalJsonCalls returns the calls to the global JSON.parse and
// JSON.stringify whose Effect context matches checkInEffect.
func findGlobalJsonCalls(ctx *rule.Context, checkInEffect bool) []globalJsonCall {
	var calls []globalJsonCall
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindCallExpression {
			inEffect := ctx.TypeParser.GetEffectContextFlags(node)&typeparser.EffectContextFlagInEffect != 0
			if inEffect == checkInEffect {
				expression := node.AsCallExpression().Expression
				for _, method := range []string{"parse", "stringify"} {
					if ctx.TypeParser.IsNodeReferenceToGlobalMember(expression, "JSON", method) {
						calls = append(calls, globalJsonCall{node: node, method: method})
						break
					}
				}
			}
		}

		node.ForEachChild(walk)
		return false
	}

	walk(ctx.SourceFile.AsNode())

	return calls
}

func globalJsonRecommendation(isV4 bool) string {
	if isV4 {
		return "Use `Schema.fromJsonString(Schema.Unknown)` for unknown shapes, `Schema.fromJsonString(schema)` for known ones, or `Schema.toCodecJson(schema)` when working with JSON values instead of strings."
	}

	return "Use `Schema.parseJson(Schema.Unknown)` for unknown shapes or `Schema.parseJson(schema)` for known ones."
}
