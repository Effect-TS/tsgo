package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var AcquireReleaseDisposableFix = fixable.Fixable{
	Name:        "acquireReleaseDisposable",
	Description: "Replace Effect.acquireRelease with Effect.acquireDisposable",
	ErrorCodes: []int32{
		tsdiag.Effect_acquireDisposable_expresses_this_disposable_resource_acquisition_more_directly_than_Effect_acquireRelease_with_a_manual_disposal_finalizer_effect_acquireReleaseDisposable.Code(),
	},
	FixIDs: []string{"acquireReleaseDisposable_fix"},
	Run:    runAcquireReleaseDisposableFix,
}

func runAcquireReleaseDisposableFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeAcquireReleaseDisposable(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.EffectModule == nil || match.HasTypeArguments {
			return nil
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.acquireDisposable",
			Run: func(tracker *rewriter.Tracker) {
				callee := tracker.NewPropertyAccessExpression(
					tracker.DeepCloneNode(match.EffectModule),
					nil,
					tracker.NewIdentifier("acquireDisposable"),
					ast.NodeFlagsNone,
				)
				replacement := tracker.NewCallExpression(
					callee,
					nil,
					nil,
					tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(match.Acquire)}),
					ast.NodeFlagsNone,
				)
				ast.SetParentInChildren(replacement)
				tracker.ReplaceNode(ctx.SourceFile, match.CallNode, replacement, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}
	return nil
}
