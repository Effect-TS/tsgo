package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var EffectSucceedWithUndefinedFix = fixable.Fixable{
	Name:        "effectSucceedWithUndefined",
	Description: "Replace with Effect.undefined",
	ErrorCodes:  []int32{tsdiag.Effect_undefined_represents_the_same_outcome_as_Effect_succeed_undefined_or_Effect_succeed_void_0_effect_effectSucceedWithUndefined.Code()},
	FixIDs:      []string{"effectSucceedWithUndefined_fix"},
	Run:         runEffectSucceedWithUndefinedFix,
}

func runEffectSucceedWithUndefinedFix(ctx *fixable.Context) []ls.CodeAction {
	return runEffectSucceedConstantFix(ctx, rules.AnalyzeEffectSucceedWithUndefined(ctx.TypeParser, ctx.Checker, ctx.SourceFile), "undefined")
}
