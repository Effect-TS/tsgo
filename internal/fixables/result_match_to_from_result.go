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

var ResultMatchToFromResultFix = fixable.Fixable{
	Name:        "resultMatchToFromResult",
	Description: "Replace Result.match or a Result guard conditional with Effect.fromResult",
	ErrorCodes: []int32{
		tsdiag.Effect_fromResult_expresses_this_Result_to_Effect_conversion_more_directly_than_Result_match_or_a_Result_guard_conditional_effect_resultMatchToFromResult.Code(),
	},
	FixIDs: []string{"resultMatchToFromResult_fix"},
	Run:    runResultMatchToFromResultFix,
}

func runResultMatchToFromResultFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeResultMatchToFromResult(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.Transformation == nil && (match.ReplacementNode == nil || match.ResultNode == nil) {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.fromResult",
			Run: func(tracker *rewriter.Tracker) {
				callee := tracker.NewPropertyAccessPath(typeparser.FindEffectModuleReference(ctx.SourceFile).Append("fromResult")...)
				if match.Transformation != nil {
					tracker.ReplacePipingFlowTransformation(ctx.SourceFile, match.Transformation, rewriter.PipingFlowTransformationReplacement{
						Callee: callee,
					})
					return
				}

				// A guard conditional is not a flow: the edit replaces the
				// ConditionalExpression around the guard flow, which no flow edit reaches.
				replacement := tracker.NewCallExpression(callee, nil, nil, tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(match.ResultNode)}), ast.NodeFlagsNone)
				ast.SetParentInChildren(replacement)
				tracker.ReplaceNode(ctx.SourceFile, match.ReplacementNode, replacement, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
