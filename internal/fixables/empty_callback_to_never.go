package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var EmptyCallbackToNeverFix = fixable.Fixable{
	Name:        "emptyCallbackToNever",
	Description: "Replace an empty Effect.callback register with Effect.never",
	ErrorCodes: []int32{
		tsdiag.This_Effect_callback_register_is_empty_so_the_effect_never_completes_Use_Effect_never_instead_effect_emptyCallbackToNever.Code(),
	},
	FixIDs: []string{"emptyCallbackToNever_fix"},
	Run:    runEmptyCallbackToNeverFix,
}

func runEmptyCallbackToNeverFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeEmptyCallbackToNever(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if match.EffectModule == nil || (!match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location)) {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.never",
			Run: func(tracker *rewriter.Tracker) {
				replacement := tracker.NewPropertyAccessExpression(tracker.DeepCloneNode(match.EffectModule), nil, tracker.NewIdentifier("never"), ast.NodeFlagsNone)
				tracker.ReplacePipingFlowPrefix(match.SourceFile, match.Flow, 1, replacement)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
