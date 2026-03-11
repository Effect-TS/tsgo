package fixables

import (
	"github.com/effect-ts/effect-typescript-go/internal/fixable"
	"github.com/effect-ts/effect-typescript-go/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var LayerMergeAllWithDependenciesFix = fixable.Fixable{
	Name:        "layerMergeAllWithDependencies",
	Description: "Move layer to Layer.provideMerge",
	ErrorCodes:  []int32{tsdiag.This_layer_provides_0_which_is_required_by_another_layer_in_the_same_Layer_mergeAll_call_Layer_mergeAll_creates_layers_in_parallel_so_dependencies_between_layers_will_not_be_satisfied_Consider_moving_this_layer_into_a_Layer_provideMerge_after_the_Layer_mergeAll_effect_layerMergeAllWithDependencies.Code()},
	FixIDs:      []string{"layerMergeAllWithDependencies_fix"},
	Run:         runLayerMergeAllWithDependenciesFix,
}

func runLayerMergeAllWithDependenciesFix(ctx *fixable.Context) []ls.CodeAction {
	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	sf := ctx.SourceFile

	matches := rules.AnalyzeLayerMergeAllWithDependencies(c, sf)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Move layer to Layer.provideMerge",
			Run: func(tracker *change.Tracker) {
				// Step A: Delete the provider argument from the mergeAll call
				if match.ProviderIndex == 0 && len(match.AllArgs) > 1 {
					// First argument with more args: delete from arg start to next arg start
					tracker.DeleteRange(sf, core.NewTextRange(match.ProviderArg.Pos(), match.AllArgs[1].Pos()))
				} else if match.ProviderIndex > 0 {
					// Not first argument: delete from previous arg end to this arg end
					tracker.DeleteRange(sf, core.NewTextRange(match.AllArgs[match.ProviderIndex-1].End(), match.ProviderArg.End()))
				}

				// Derive the Layer module node from the call expression
				if match.CallNode.Kind != ast.KindCallExpression {
					return
				}
				call := match.CallNode.AsCallExpression()
				if call.Expression.Kind != ast.KindPropertyAccessExpression {
					return
				}
				layerModuleNode := call.Expression.AsPropertyAccessExpression().Expression

				// Step B: Build Layer.provideMerge(providerArg) call expression
				clonedProviderArg := tracker.DeepCloneNode(match.ProviderArg)
				clonedLayerModule := tracker.DeepCloneNode(layerModuleNode)

				provideMergeAccess := tracker.NewPropertyAccessExpression(
					clonedLayerModule,
					nil,
					tracker.NewIdentifier("provideMerge"),
					ast.NodeFlagsNone,
				)

				provideMergeCall := tracker.NewCallExpression(
					provideMergeAccess,
					nil,
					nil,
					tracker.NewNodeList([]*ast.Node{clonedProviderArg}),
					ast.NodeFlagsNone,
				)

				ast.SetParentInChildren(provideMergeCall)

				// Step C: Insert .pipe(Layer.provideMerge(...)) at the end of the call
				// Use two separate operations because the Suffix dedup logic in the
				// change tracker skips ")" when the formatted call already ends with ")".
				tracker.InsertNodeAt(sf, core.TextPos(match.CallNode.End()), provideMergeCall, change.NodeOptions{
					Prefix: ".pipe(",
				})
				tracker.InsertText(sf, ctx.BytePosToLSPPosition(match.CallNode.End()), ")")
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
