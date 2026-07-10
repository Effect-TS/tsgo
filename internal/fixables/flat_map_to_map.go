package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
)

var FlatMapToMapFix = fixable.Fixable{
	Name:        "flatMapToMap",
	Description: "Replace Effect.flatMap + Effect.succeed with Effect.map",
	ErrorCodes: []int32{
		tsdiag.Effect_map_expresses_this_success_value_transformation_more_directly_than_Effect_flatMap_followed_by_Effect_succeed_effect_flatMapToMap.Code(),
	},
	FixIDs: []string{"flatMapToMap_fix"},
	Run:    runFlatMapToMapFix,
}

func runFlatMapToMapFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile

	matches := rules.AnalyzeFlatMapToMap(ctx.TypeParser, ctx.Checker, sf)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.map",
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceNode(sf, match.CalleeNameNode, tracker.NewIdentifier("map"), nil)
				tracker.ReplaceNode(sf, match.SucceedCallExpression, match.SucceedArgument, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
