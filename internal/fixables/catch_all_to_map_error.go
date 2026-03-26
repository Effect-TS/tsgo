package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var CatchAllToMapErrorFix = fixable.Fixable{
	Name:        "catchAllToMapError",
	Description: "Replace Effect.catch + Effect.fail with Effect.mapError",
	ErrorCodes:  []int32{tsdiag.You_can_use_Effect_mapError_instead_of_Effect_catch_Effect_fail_to_transform_the_error_type_effect_catchAllToMapError.Code()},
	FixIDs:      []string{"catchAllToMapError_fix"},
	Run:         runCatchAllToMapErrorFix,
}

func runCatchAllToMapErrorFix(ctx *fixable.Context) []ls.CodeAction {

	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	sf := ctx.SourceFile

	matches := rules.AnalyzeCatchAllToMapError(c, sf)
	for _, match := range matches {
		diagRange := match.Location
		if !diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.mapError",
			Run: func(tracker *change.Tracker) {
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
