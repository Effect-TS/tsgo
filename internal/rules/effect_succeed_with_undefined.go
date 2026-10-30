package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

// EffectSucceedWithUndefined suggests using Effect.undefined instead of Effect.succeed(undefined) or Effect.succeed(void 0).
var EffectSucceedWithUndefined = rule.Rule{
	Name:            "effectSucceedWithUndefined",
	Group:           "style",
	Description:     "Suggests using Effect.undefined instead of Effect.succeed(undefined) or Effect.succeed(void 0)",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.Effect_undefined_represents_the_same_outcome_as_Effect_succeed_undefined_or_Effect_succeed_void_0_effect_effectSucceedWithUndefined.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeEffectSucceedWithUndefined(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Effect_undefined_represents_the_same_outcome_as_Effect_succeed_undefined_or_Effect_succeed_void_0_effect_effectSucceedWithUndefined, nil)
		}
		return diags
	},
}

// AnalyzeEffectSucceedWithUndefined finds Effect.succeed(undefined) and Effect.succeed(void 0)
// steps whose success type is exactly undefined, which Effect.undefined replaces. Effect.undefined
// only exists in Effect v4.
func AnalyzeEffectSucceedWithUndefined(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []EffectSucceedConstantMatch {
	if tp == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}
	return filterEffectSucceedConstantMatches(analyzeEffectSucceedConstant(tp, sf), "undefined")
}
