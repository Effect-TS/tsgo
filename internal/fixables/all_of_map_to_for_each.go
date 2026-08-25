package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var AllOfMapToForEachFix = fixable.Fixable{
	Name:        "allOfMapToForEach",
	Description: "Replace Effect.all over Array#map with Effect.forEach",
	ErrorCodes: []int32{
		tsdiag.Effect_forEach_expresses_this_effectful_array_mapping_more_directly_than_Effect_all_over_Array_map_effect_allOfMapToForEach.Code(),
	},
	FixIDs: []string{"allOfMapToForEach_fix"},
	Run:    runAllOfMapToForEachFix,
}

func runAllOfMapToForEachFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeAllOfMapToForEach(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		// The generic parameter lists of Array#map, Effect.all, and Effect.forEach
		// differ, so an explicit type argument cannot be moved safely. A bare
		// imported `all` also cannot be renamed without changing its import.
		if match.HasTypeArguments || !match.CanFixReceiver || match.EffectModule == nil {
			return nil
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.forEach",
			Run: func(tracker *rewriter.Tracker) {
				callee := tracker.NewPropertyAccessExpression(
					tracker.DeepCloneNode(match.EffectModule),
					nil,
					tracker.NewIdentifier("forEach"),
					ast.NodeFlagsNone,
				)
				arguments := []*ast.Node{
					tracker.DeepCloneNode(match.Receiver),
					tracker.DeepCloneNode(match.Callback),
				}
				if match.Options != nil {
					arguments = append(arguments, tracker.DeepCloneNode(match.Options))
				}
				replacement := tracker.NewCallExpression(callee, nil, nil, tracker.NewNodeList(arguments), ast.NodeFlagsNone)
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
