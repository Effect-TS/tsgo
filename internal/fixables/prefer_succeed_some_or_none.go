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
		if match.Flow == nil || match.TransformationCount <= 0 {
			continue
		}
		succeed := match.Flow.Transformations().At(match.TransformationCount - 1)
		if succeed.Kind() != typeparser.TransformationKindCall &&
			succeed.Kind() != typeparser.TransformationKindPipe &&
			succeed.Kind() != typeparser.TransformationKindPipeable {
			continue
		}
		var value *ast.Node
		var valueTypeArguments *ast.NodeList
		if match.ReplacementName == "succeedSome" {
			optionIndex := match.TransformationCount - 2
			value = match.Flow.TransformationInputNode(optionIndex)
			valueTypeArguments = match.Flow.Transformations().At(optionIndex).TypeArguments()
			if value == nil {
				continue
			}
		}

		description := "Replace with Effect." + match.ReplacementName
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: description,
			Run: func(tracker *rewriter.Tracker) {
				callee := tracker.NewPropertyAccessPath(typeparser.FindEffectModuleReference(ctx.SourceFile).Append(match.ReplacementName)...)
				replacement := callee
				if value != nil {
					var typeArguments *ast.NodeList
					if valueTypeArguments != nil && len(valueTypeArguments.Nodes) > 0 {
						arguments := make([]*ast.Node, len(valueTypeArguments.Nodes))
						for i, argument := range valueTypeArguments.Nodes {
							arguments[i] = tracker.DeepCloneNode(argument)
						}
						typeArguments = tracker.NewNodeList(arguments)
					}
					replacement = tracker.NewCallExpression(
						callee,
						nil,
						typeArguments,
						tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(value)}),
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
