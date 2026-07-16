package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
)

var SyncToSucceedFix = fixable.Fixable{
	Name:        "syncToSucceed",
	Description: "Replace Effect.sync with Effect.succeed",
	ErrorCodes: []int32{
		tsdiag.Effect_succeed_expresses_this_constant_value_more_directly_than_Effect_sync_effect_syncToSucceed.Code(),
	},
	FixIDs: []string{"syncToSucceed_fix"},
	Run:    runSyncToSucceedFix,
}

func runSyncToSucceedFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeSyncToSucceed(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.succeed",
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceNode(ctx.SourceFile, match.CalleeName, tracker.NewIdentifier("succeed"), nil)
				tracker.ReplaceNode(ctx.SourceFile, match.Thunk, match.ConstantValue, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
