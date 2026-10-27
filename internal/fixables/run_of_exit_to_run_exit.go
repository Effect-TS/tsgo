package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var RunOfExitToRunExitFix = fixable.Fixable{
	Name:        "runOfExitToRunExit",
	Description: "Replace with the dedicated Effect Exit runner",
	ErrorCodes: []int32{
		tsdiag.Effect_0_of_Effect_exit_re_implements_Effect_1_Use_the_dedicated_Exit_runner_directly_effect_runOfExitToRunExit.Code(),
	},
	FixIDs: []string{"runOfExitToRunExit_fix"},
	Run:    runRunOfExitToRunExitFix,
}

func runRunOfExitToRunExitFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeRunOfExitToRunExit(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.RunnerCallee == nil || match.ExitTransformation == nil {
			continue
		}

		description := "Replace with Effect." + match.ReplacementName
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: description,
			Run: func(tracker *rewriter.Tracker) {
				if !removePipingFlowTransformation(tracker, ctx.SourceFile, match.ExitTransformation) {
					return
				}
				replaceEffectMethodCallee(tracker, ctx.SourceFile, match.RunnerCallee, match.RunnerNameNode, match.ReplacementName)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		continue
	}

	return nil
}
