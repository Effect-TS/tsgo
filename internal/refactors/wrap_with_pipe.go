package refactors

import (
	"github.com/effect-ts/tsgo/internal/refactor"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var WrapWithPipe = refactor.Refactor{
	Name:        "wrapWithPipe",
	Description: "Wrap with pipe",
	Kind:        "rewrite.effect.wrapWithPipe",
	Run:         runWrapWithPipe,
}

func runWrapWithPipe(ctx *refactor.Context) []ls.CodeAction {
	// Only applicable when the selection is non-empty.
	if ctx.Span.Pos() == ctx.Span.End() {
		return nil
	}

	startPos := ctx.BytePosToLSPPosition(ctx.Span.Pos())
	endPos := ctx.BytePosToLSPPosition(ctx.Span.End())

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Wrap with pipe",
		Run: func(tracker *change.Tracker) {
			tracker.InsertText(ctx.SourceFile, startPos, "pipe(")
			tracker.InsertText(ctx.SourceFile, endPos, ")")
		},
	})
	if action == nil {
		return nil
	}

	action.Kind = "refactor.rewrite.effect.wrapWithPipe"
	return []ls.CodeAction{*action}
}
