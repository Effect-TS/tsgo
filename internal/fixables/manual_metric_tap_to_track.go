package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var ManualMetricTapToTrackFix = fixable.Fixable{
	Name:        "manualMetricTapToTrack",
	Description: "Replace Effect.tap with Effect.track*",
	ErrorCodes: []int32{
		tsdiag.Effect_0_expresses_recording_a_metric_on_every_1_directly_effect_manualMetricTapToTrack.Code(),
	},
	FixIDs: []string{"manualMetricTapToTrack_fix"},
	Run:    runManualMetricTapToTrackFix,
}

func runManualMetricTapToTrackFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeManualMetricTapToTrack(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.Transformation == nil {
			continue
		}
		// Only offer the fix when every generated reference resolves at the call site.
		callee := match.Transformation.Callee()
		track := effectModuleMember(ctx, match.TrackAPI)
		trackReference := resolveModuleMember(ctx, callee, typeparser.FindEffectModuleReference(ctx.SourceFile), track)
		if trackReference == nil {
			continue
		}
		withConstantInput := metricModuleMember(ctx, "withConstantInput")
		var withConstantInputReference *moduleMemberReference
		if match.Kind == rules.ManualMetricTapToTrackConstantInput {
			withConstantInputReference = resolveModuleMember(ctx, callee, typeparser.FindModuleReference(ctx.SourceFile, "Metric"), withConstantInput)
			if withConstantInputReference == nil {
				continue
			}
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.TrackAPI,
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplacePipingFlowTransformation(ctx.SourceFile, match.Transformation, rewriter.PipingFlowTransformationReplacement{
					Callee: trackReference.expression(tracker, track),
					Arguments: tracker.NewNodeList(manualMetricTrackArguments(tracker, match, func() *ast.Node {
						return withConstantInputReference.expression(tracker, withConstantInput)
					})),
				})
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}

func effectModuleMember(ctx *fixable.Context, name string) moduleMember {
	return moduleMember{
		name:     name,
		isModule: ctx.TypeParser.IsExpressionEffectModule,
		isMember: func(node *ast.Node) bool { return ctx.TypeParser.IsNodeReferenceToEffectModuleApi(node, name) },
	}
}

func metricModuleMember(ctx *fixable.Context, name string) moduleMember {
	return moduleMember{
		name:     name,
		isModule: ctx.TypeParser.IsExpressionEffectMetricModule,
		isMember: func(node *ast.Node) bool { return ctx.TypeParser.IsNodeReferenceToEffectMetricModuleApi(node, name) },
	}
}

func manualMetricTrackArguments(tracker *rewriter.Tracker, match rules.ManualMetricTapToTrackMatch, withConstantInput func() *ast.Node) []*ast.Node {
	metric := tracker.DeepCloneNode(match.Metric)
	switch match.Kind {
	case rules.ManualMetricTapToTrackMapper:
		params := make([]*ast.Node, 0, match.Params.Len())
		for _, param := range match.Params.All() {
			params = append(params, tracker.DeepCloneNode(param))
		}
		mapper := tracker.NewArrowFunction(
			nil,
			nil,
			tracker.NewNodeList(params),
			nil,
			nil,
			tracker.NewToken(ast.KindEqualsGreaterThanToken),
			tracker.DeepCloneNode(match.Input),
		)
		return []*ast.Node{metric, mapper}
	case rules.ManualMetricTapToTrackConstantInput:
		call := tracker.NewCallExpression(
			withConstantInput(),
			nil,
			nil,
			tracker.NewNodeList([]*ast.Node{metric, tracker.DeepCloneNode(match.Input)}),
			ast.NodeFlagsNone,
		)
		return []*ast.Node{call}
	}
	return []*ast.Node{metric}
}
