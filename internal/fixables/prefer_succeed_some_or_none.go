package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var PreferSucceedSomeOrNoneFix = fixable.Fixable{
	Name:        "preferSucceedSomeOrNone",
	Description: "Replace with the direct Effect Option constructor",
	ErrorCodes: []int32{
		tsdiag.Effect_0_expresses_this_Option_success_value_directly_effect_preferSucceedSomeOrNone.Code(),
	},
	FixIDs: []string{"preferSucceedSomeOrNone_fix"},
	Run:    runPreferSucceedSomeOrNoneFix,
}

func runPreferSucceedSomeOrNoneFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzePreferSucceedSomeOrNone(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.Flow == nil || match.TransformationCount <= 0 || match.ReplacementName == "succeedSome" && match.ValueNode == nil {
			return nil
		}

		description := "Replace with Effect." + match.ReplacementName
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: description,
			Run: func(tracker *rewriter.Tracker) {
				callee := tracker.NewPropertyAccessExpression(
					tracker.DeepCloneNode(match.EffectModuleNode),
					nil,
					tracker.NewIdentifier(match.ReplacementName),
					ast.NodeFlagsNone,
				)
				replacement := callee
				if match.ValueNode != nil {
					var typeArguments *ast.NodeList
					if match.ValueTypeArguments != nil && len(match.ValueTypeArguments.Nodes) > 0 {
						arguments := make([]*ast.Node, len(match.ValueTypeArguments.Nodes))
						for i, argument := range match.ValueTypeArguments.Nodes {
							arguments[i] = tracker.DeepCloneNode(argument)
						}
						typeArguments = tracker.NewNodeList(arguments)
					}
					replacement = tracker.NewCallExpression(
						callee,
						nil,
						typeArguments,
						tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(match.ValueNode)}),
						ast.NodeFlagsNone,
					)
				}
				tracker.ReplacePipingFlowPrefix(ctx.SourceFile, match.Flow, match.TransformationCount, replacement)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
