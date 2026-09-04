package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var MatchEffectToMapBothFix = fixable.Fixable{
	Name: "matchEffectToMapBoth", Description: "Replace Effect.matchEffect with Effect.mapBoth and unwrap Effect.fail and Effect.succeed handlers",
	ErrorCodes: []int32{tsdiag.Effect_mapBoth_expresses_these_failure_and_success_transformations_more_directly_than_Effect_matchEffect_effect_matchEffectToMapBoth.Code()},
	FixIDs:     []string{"matchEffectToMapBoth_fix"}, Run: runMatchEffectToMapBothFix,
}

func runMatchEffectToMapBothFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeMatchEffectToMapBoth(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.mapBoth",
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceNode(match.SourceFile, match.CalleeNameNode, tracker.NewIdentifier("mapBoth"), nil)
				for i := range match.HandlerResults {
					tracker.ReplaceNode(match.SourceFile, match.HandlerResults[i], match.ConstructorArgs[i], nil)
				}
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
