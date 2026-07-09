package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
)

var CatchAllToMapErrorFix = fixable.Fixable{
	Name:        "catchAllToMapError",
	Description: "Replace Effect.catch + Effect.fail with Effect.mapError",
	ErrorCodes:  []int32{tsdiag.Effect_mapError_expresses_the_same_error_type_transformation_more_directly_than_Effect_0_followed_by_Effect_fail_effect_catchAllToMapError.Code()},
	FixIDs:      []string{"catchAllToMapError_fix"},
	Run:         runCatchAllToMapErrorFix,
}

func runCatchAllToMapErrorFix(ctx *fixable.Context) []ls.CodeAction {

	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeCatchAllToMapError(ctx.TypeParser, c, sf)
	for _, match := range matches {
		diagRange := match.Location
		if !diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.mapError",
			Run: func(tracker *rewriter.Tracker) {
				// Edit 1: Replace "catch" with "mapError" in the callee
				if match.CalleeNameNode != nil {
					tracker.ReplaceNode(sf, match.CalleeNameNode, tracker.NewIdentifier("mapError"), nil)
				}

				// Edit 2: Unwrap "Effect.fail(arg)" to "arg" by deleting prefix and suffix
				tracker.DeleteRange(sf, core.NewTextRange(match.FailCallExpression.Pos(), match.FailArgument.Pos()))
				tracker.DeleteRange(sf, core.NewTextRange(match.FailArgument.End(), match.FailCallExpression.End()))
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
