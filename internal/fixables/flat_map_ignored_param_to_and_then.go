package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var FlatMapIgnoredParamToAndThenFix = fixable.Fixable{
	Name:        "flatMapIgnoredParamToAndThen",
	Description: "Replace Effect.flatMap with Effect.andThen",
	ErrorCodes: []int32{
		tsdiag.Effect_andThen_expresses_this_sequencing_more_directly_than_Effect_flatMap_with_a_zero_parameter_callback_effect_flatMapIgnoredParamToAndThen.Code(),
	},
	FixIDs: []string{"flatMapIgnoredParamToAndThen_fix"},
	Run:    runFlatMapIgnoredParamToAndThenFix,
}

func runFlatMapIgnoredParamToAndThenFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeFlatMapIgnoredParamToAndThen(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.andThen",
			Run: func(tracker *rewriter.Tracker) {
				replaceEffectMethodCallee(tracker, ctx.SourceFile, match.Callee, match.CalleeNameNode, "andThen")
				tracker.ReplaceNode(ctx.SourceFile, match.Callback, match.EffectValue, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
