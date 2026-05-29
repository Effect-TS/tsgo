package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
)

var CatchToOrElseSucceedFix = fixable.Fixable{
	Name:        "catchToOrElseSucceed",
	Description: "Replace Effect.catch + Effect.succeed with Effect.orElseSucceed",
	ErrorCodes:  []int32{tsdiag.Effect_orElseSucceed_expresses_the_same_recovery_more_directly_than_Effect_0_followed_by_Effect_succeed_effect_catchToOrElseSucceed.Code()},
	FixIDs:      []string{"catchToOrElseSucceed_fix"},
	Run:         runCatchToOrElseSucceedFix,
}

func runCatchToOrElseSucceedFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile

	matches := rules.AnalyzeCatchToOrElseSucceed(ctx.TypeParser, ctx.Checker, sf)
	for _, match := range matches {
		diagRange := match.Location
		if !diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.orElseSucceed",
			Run: func(tracker *rewriter.Tracker) {
				if match.CalleeNameNode != nil {
					tracker.ReplaceNode(sf, match.CalleeNameNode, tracker.NewIdentifier("orElseSucceed"), nil)
				}

				tracker.ReplaceNode(sf, match.SucceedCallExpression, match.SucceedArgument, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
