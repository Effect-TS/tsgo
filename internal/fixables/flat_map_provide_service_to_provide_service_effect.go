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

var FlatMapProvideServiceToProvideServiceEffectFix = fixable.Fixable{
	Name:        "flatMapProvideServiceToProvideServiceEffect",
	Description: "Replace Effect.flatMap or Effect.andThen with Effect.provideServiceEffect",
	ErrorCodes: []int32{
		tsdiag.Effect_provideServiceEffect_provides_this_service_from_the_acquiring_effect_directly_instead_of_Effect_0_forwarding_its_parameter_to_Effect_provideService_effect_flatMapProvideServiceToProvideServiceEffect.Code(),
	},
	FixIDs: []string{"flatMapProvideServiceToProvideServiceEffect_fix"},
	Run:    runFlatMapProvideServiceToProvideServiceEffectFix,
}

func runFlatMapProvideServiceToProvideServiceEffectFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeFlatMapProvideServiceToProvideServiceEffect(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		provideServiceEffect := effectModuleMember(ctx, "provideServiceEffect")
		target := resolveModuleMember(ctx, match.Flow.Node(), typeparser.FindEffectModuleReference(match.SourceFile), provideServiceEffect)
		if target == nil {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.provideServiceEffect",
			Run: func(tracker *rewriter.Tracker) {
				self := tracker.PipingFlowPrefixExpression(match.Inner, match.Inner.Transformations().Len()-1)
				acquire := tracker.PipingFlowPrefixExpression(match.Flow, match.Index)
				if self == nil || acquire == nil {
					return
				}
				replacement := tracker.NewCallExpression(
					target.expression(tracker, provideServiceEffect),
					nil,
					nil,
					tracker.NewNodeList([]*ast.Node{self, tracker.DeepCloneNode(match.Key), acquire}),
					ast.NodeFlagsNone,
				)
				tracker.ReplacePipingFlowPrefix(match.SourceFile, match.Flow, match.Index+1, replacement)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
