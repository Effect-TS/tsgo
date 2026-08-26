package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var CatchDieToOrDieFix = fixable.Fixable{
	Name:        "catchDieToOrDie",
	Description: "Replace the catch-all Effect.die handler with Effect.orDie",
	ErrorCodes: []int32{
		tsdiag.Effect_orDie_expresses_escalating_every_typed_failure_into_a_defect_more_directly_than_Effect_0_with_an_identity_forwarding_Effect_die_handler_effect_catchDieToOrDie.Code(),
	},
	FixIDs: []string{"catchDieToOrDie_fix"},
	Run:    runCatchDieToOrDieFix,
}

func runCatchDieToOrDieFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeCatchDieToOrDie(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.EffectModule == nil || match.Transformation == nil || match.HasTypeArguments ||
			(match.IsDataApplication && match.Input == nil) {
			return nil
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.orDie",
			Run: func(tracker *rewriter.Tracker) {
				orDie := tracker.NewPropertyAccessExpression(
					tracker.DeepCloneNode(match.EffectModule),
					nil,
					tracker.NewIdentifier("orDie"),
					ast.NodeFlagsNone,
				)
				replacement := orDie
				if match.IsDataApplication {
					replacement = tracker.NewCallExpression(
						orDie,
						nil,
						nil,
						tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(match.Input)}),
						ast.NodeFlagsNone,
					)
				}
				ast.SetParentInChildren(replacement)
				tracker.ReplaceNode(ctx.SourceFile, match.Transformation, replacement, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}
	return nil
}
