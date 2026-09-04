package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var MatchEffectToMatchFix = fixable.Fixable{
	Name: "matchEffectToMatch", Description: "Replace Effect.matchEffect with Effect.match and unwrap Effect.succeed handlers",
	ErrorCodes: []int32{tsdiag.Effect_0_expresses_this_non_effectful_fold_more_directly_than_Effect_1_with_Effect_succeed_handlers_effect_matchEffectToMatch.Code()},
	FixIDs:     []string{"matchEffectToMatch_fix"}, Run: runMatchEffectToMatchFix,
}

func runMatchEffectToMatchFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeMatchEffectToMatch(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.ReplacementName,
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceNode(match.SourceFile, match.CalleeNameNode, tracker.NewIdentifier(match.ReplacementName), nil)
				for i := range match.HandlerResults {
					tracker.ReplaceNode(match.SourceFile, match.HandlerResults[i], match.SucceedArguments[i], nil)
				}
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
