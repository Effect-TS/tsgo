// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

// PreferSchemaOverJson is deprecated: it reports exactly what GlobalJsonInEffect
// reports, with its original message, so configurations that enable it keep
// their coverage.
var PreferSchemaOverJson = rule.Rule{
	Name:            "preferSchemaOverJson",
	Group:           "effectNative",
	Description:     "Deprecated: use globalJsonInEffect and globalJson instead. Suggests using Effect Schema for JSON operations instead of JSON.parse/JSON.stringify inside Effect code",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_code_uses_JSON_parse_or_JSON_stringify_0_effect_preferSchemaOverJson.Code()},
	ReplacedBy:      []string{"globalJsonInEffect", "globalJson"},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		recommendation := globalJsonRecommendation(ctx.TypeParser.SupportedEffectVersion() == typeparser.EffectMajorV4)
		var diags []*ast.Diagnostic
		for _, call := range findGlobalJsonCalls(ctx, true) {
			diags = append(diags, ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(call.node), tsdiag.This_code_uses_JSON_parse_or_JSON_stringify_0_effect_preferSchemaOverJson, nil, recommendation))
		}
		return diags
	},
}
