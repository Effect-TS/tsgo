package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var MultipleEffectProvideFix = fixable.Fixable{
	Name:        "multipleEffectProvide",
	Description: "Combine into a single provide",
	ErrorCodes:  []int32{tsdiag.Avoid_chaining_Effect_provide_calls_as_this_can_lead_to_service_lifecycle_issues_Instead_merge_layers_and_provide_them_in_a_single_call_effect_multipleEffectProvide.Code()},
	FixIDs:      []string{"multipleEffectProvide_fix"},
	Run:         runMultipleEffectProvideFix,
}

func runMultipleEffectProvideFix(ctx *fixable.Context) []ls.CodeAction {
	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	sf := ctx.SourceFile

	var actions []ls.CodeAction

	matches := rules.AnalyzeMultipleEffectProvide(c, sf)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		// Resolve the Effect module name, preserving import aliases
		effectModuleName := "Effect"
		if match.EffectModuleNode != nil && match.EffectModuleNode.Kind == ast.KindIdentifier {
			effectModuleName = scanner.GetTextOfNode(match.EffectModuleNode)
		}

		// Resolve the Layer module name from the source file imports
		layerModuleName := typeparser.FindModuleIdentifier(sf, "Layer")

		// Capture loop variables for the closure
		chunk := match.Chunk
		layerArgs := match.LayerArgs

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Combine into a single provide",
			Run: func(tracker *change.Tracker) {
				// Step 1: Delete the range spanning all consecutive provide call expressions
				tokenPos := scanner.GetTokenPosOfNode(chunk[0], sf, false)
				endPos := chunk[len(chunk)-1].End()
				tracker.DeleteRange(sf, core.NewTextRange(tokenPos, endPos))

				// Step 2: Build Effect.provide(Layer.mergeAll(arg1, arg2, ...))
				effectId := tracker.NewIdentifier(effectModuleName)
				provideAccess := tracker.NewPropertyAccessExpression(
					effectId, nil, tracker.NewIdentifier("provide"), ast.NodeFlagsNone,
				)

				layerId := tracker.NewIdentifier(layerModuleName)
				mergeAllAccess := tracker.NewPropertyAccessExpression(
					layerId, nil, tracker.NewIdentifier("mergeAll"), ast.NodeFlagsNone,
				)

				// Clone all layer arguments
				clonedArgs := make([]*ast.Node, len(layerArgs))
				for i, arg := range layerArgs {
					clonedArgs[i] = tracker.DeepCloneNode(arg)
				}

				mergeAllCall := tracker.NewCallExpression(
					mergeAllAccess, nil, nil,
					tracker.NewNodeList(clonedArgs),
					ast.NodeFlagsNone,
				)

				provideCall := tracker.NewCallExpression(
					provideAccess, nil, nil,
					tracker.NewNodeList([]*ast.Node{mergeAllCall}),
					ast.NodeFlagsNone,
				)

				ast.SetParentInChildren(provideCall)

				// Step 3: Insert the new node at the position of the first deleted call
				tracker.InsertNodeAt(sf, core.TextPos(tokenPos), provideCall, change.NodeOptions{})
			},
		}); action != nil {
			actions = append(actions, *action)
		}
	}

	return actions
}
