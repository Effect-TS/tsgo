package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var ProvideLayerSucceedToProvideServiceFix = fixable.Fixable{
	Name:        "provideLayerSucceedToProvideService",
	Description: "Replace an inline single-service layer with direct service provision",
	ErrorCodes: []int32{
		tsdiag.Effect_0_provides_this_inline_single_service_layer_directly_effect_provideLayerSucceedToProvideService.Code(),
	},
	FixIDs: []string{"provideLayerSucceedToProvideService_fix"},
	Run:    runProvideLayerSucceedToProvideServiceFix,
}

func runProvideLayerSucceedToProvideServiceFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeProvideLayerSucceedToProvideService(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.ProvideTransformation == nil || match.EffectModuleNode == nil {
			return nil
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.ReplacementMethodName,
			Run: func(tracker *rewriter.Tracker) {
				callee := tracker.NewPropertyAccessExpression(
					tracker.DeepCloneNode(match.EffectModuleNode),
					nil,
					tracker.NewIdentifier(match.ReplacementMethodName),
					ast.NodeFlagsNone,
				)
				arguments := tracker.NewNodeList([]*ast.Node{
					tracker.DeepCloneNode(match.ServiceNode),
					tracker.DeepCloneNode(match.ImplementationNode),
				})
				tracker.ReplacePipingFlowTransformation(ctx.SourceFile, match.ProvideTransformation, rewriter.PipingFlowTransformationReplacement{
					Callee:    callee,
					Arguments: arguments,
				})
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
