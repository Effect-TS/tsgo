package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var FailCauseToConstructorFix = fixable.Fixable{
	Name:        "failCauseToConstructor",
	Description: "Replace with the direct Effect failure constructor",
	ErrorCodes: []int32{
		tsdiag.Effect_0_expresses_this_failure_directly_instead_of_wrapping_Cause_2_in_Effect_1_effect_failCauseToConstructor.Code(),
	},
	FixIDs: []string{"failCauseToConstructor_fix"},
	Run:    runFailCauseToConstructorFix,
}

func runFailCauseToConstructorFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	for _, match := range rules.AnalyzeFailCauseToConstructor(ctx.TypeParser, ctx.Checker, sf) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.Flow == nil && match.LazyCauseNode == nil {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.ReplacementName,
			Run: func(tracker *rewriter.Tracker) {
				if match.Flow != nil {
					callee := effectModuleMethod(tracker, sf, match.EffectModuleNode, match.ReplacementName)
					var typeArguments *ast.NodeList
					if match.ValueTypeArguments != nil && len(match.ValueTypeArguments.Nodes) > 0 {
						arguments := make([]*ast.Node, len(match.ValueTypeArguments.Nodes))
						for i, argument := range match.ValueTypeArguments.Nodes {
							arguments[i] = tracker.DeepCloneNode(argument)
						}
						typeArguments = tracker.NewNodeList(arguments)
					}
					replacement := tracker.NewCallExpression(
						callee,
						nil,
						typeArguments,
						tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(match.ValueNode)}),
						ast.NodeFlagsNone,
					)
					tracker.ReplacePipingFlowPrefix(sf, match.Flow, match.TransformationCount, replacement)
					return
				}

				value := tracker.DeepCloneNode(match.ValueNode)
				// An object literal as a concise arrow body would parse as a block.
				if match.LazyExpressionBody && ast.IsObjectLiteralExpression(ast.SkipParentheses(match.ValueNode)) {
					value = tracker.NewParenthesizedExpression(value)
				}
				tracker.ReplaceNode(sf, match.LazyCauseNode, value, nil)
				var name *ast.Node
				if match.LazyCallee.Kind == ast.KindPropertyAccessExpression {
					name = match.LazyCallee.AsPropertyAccessExpression().Name()
				}
				replaceEffectMethodCallee(tracker, sf, match.LazyCallee, name, match.ReplacementName)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
