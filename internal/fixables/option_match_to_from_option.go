package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var OptionMatchToFromOptionFix = fixable.Fixable{
	Name:        "optionMatchToFromOption",
	Description: "Replace Option.match or an Option tag conditional with Effect.fromOption",
	ErrorCodes: []int32{
		tsdiag.Effect_fromOption_expresses_this_Option_to_Effect_conversion_more_directly_than_Option_match_or_an_Option_tag_conditional_effect_optionMatchToFromOption.Code(),
	},
	FixIDs: []string{"optionMatchToFromOption_fix"},
	Run:    runOptionMatchToFromOptionFix,
}

func runOptionMatchToFromOptionFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeOptionMatchToFromOption(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if !match.CanFix || match.EffectModuleNode == nil ||
			match.Transformation == nil && (match.ReplacementNode == nil || match.OptionNode == nil) {
			return nil
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.fromOption",
			Run: func(tracker *rewriter.Tracker) {
				callee, arguments := buildFromOptionReplacement(tracker, match)
				if callee == nil {
					return
				}
				if match.Transformation != nil {
					tracker.ReplacePipingFlowTransformation(ctx.SourceFile, match.Transformation, rewriter.PipingFlowTransformationReplacement{
						Callee:    callee,
						Arguments: arguments,
					})
					return
				}

				directArguments := []*ast.Node{tracker.DeepCloneNode(match.OptionNode)}
				if arguments != nil {
					directArguments = append(directArguments, arguments.Nodes...)
				}
				replacement := tracker.NewCallExpression(callee, nil, nil, tracker.NewNodeList(directArguments), ast.NodeFlagsNone)
				ast.SetParentInChildren(replacement)
				tracker.ReplaceNode(ctx.SourceFile, match.ReplacementNode, replacement, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

func buildFromOptionReplacement(tracker *rewriter.Tracker, match rules.OptionMatchToFromOptionMatch) (*ast.Node, *ast.NodeList) {
	fromOption := tracker.NewPropertyAccessExpression(
		tracker.DeepCloneNode(match.EffectModuleNode),
		nil,
		tracker.NewIdentifier("fromOption"),
		ast.NodeFlagsNone,
	)

	if match.DefaultFailure {
		return fromOption, nil
	}

	return fromOption, tracker.NewNodeList([]*ast.Node{
		tracker.NewArrowFunction(
			nil,
			nil,
			tracker.NewNodeList(nil),
			nil,
			nil,
			tracker.NewToken(ast.KindEqualsGreaterThanToken),
			tracker.DeepCloneNode(match.FailureNode),
		),
	})
}
