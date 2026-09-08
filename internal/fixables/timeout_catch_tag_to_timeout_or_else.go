package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var TimeoutCatchTagToTimeoutOrElseFix = fixable.Fixable{
	Name:        "timeoutCatchTagToTimeoutOrElse",
	Description: "Replace with a dedicated timeout combinator",
	ErrorCodes:  []int32{tsdiag.Use_Effect_0_to_handle_this_timeout_directly_effect_timeoutCatchTagToTimeoutOrElse.Code()},
	FixIDs:      []string{"timeoutCatchTagToTimeoutOrElse_fix"},
	Run: func(ctx *fixable.Context) []ls.CodeAction {
		for _, match := range rules.AnalyzeTimeoutCatchTagToTimeoutOrElse(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
			if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
				continue
			}
			if match.EffectModule == nil {
				return nil
			}
			action := ctx.NewFixAction(fixable.FixAction{
				Description: "Replace with Effect." + match.ReplacementName,
				Run: func(t *rewriter.Tracker) {
					callee := t.NewPropertyAccessExpression(t.DeepCloneNode(match.EffectModule), nil, t.NewIdentifier(match.ReplacementName), ast.NodeFlagsNone)
					argument := t.DeepCloneNode(match.Duration)
					if match.ReplacementName == "timeoutOrElse" {
						handler := t.DeepCloneNode(match.Handler)
						arrow := ast.SkipParentheses(match.Handler).AsArrowFunction()
						if arrow.Parameters != nil && len(arrow.Parameters.Nodes) > 0 {
							handler = t.NewArrowFunction(nil, nil, t.NewNodeList(nil), t.DeepCloneNode(arrow.Type), nil, t.NewToken(ast.KindEqualsGreaterThanToken), t.DeepCloneNode(arrow.Body))
						}
						argument = t.NewObjectLiteralExpression(t.NewNodeList([]*ast.Node{
							t.NewPropertyAssignment(nil, t.NewIdentifier("duration"), nil, nil, argument),
							t.NewPropertyAssignment(nil, t.NewIdentifier("orElse"), nil, nil, handler),
						}), true)
					}
					t.CollapsePipingFlowTransformations(ctx.SourceFile, match.Flow, match.Start, match.End, rewriter.PipingFlowTransformationReplacement{Callee: callee, Arguments: t.NewNodeList([]*ast.Node{argument})})
				},
			})
			if action != nil {
				return []ls.CodeAction{*action}
			}
			return nil
		}
		return nil
	},
}
