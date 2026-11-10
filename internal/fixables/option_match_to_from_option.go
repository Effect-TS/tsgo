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
		// A function expression thunk can read its own this or arguments, which
		// the arrow function the fix writes would not bind.
		if match.FailureHandler != nil && match.FailureHandler.Kind != ast.KindArrowFunction ||
			match.Transformation == nil && (match.ReplacementNode == nil || match.OptionNode == nil) {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.fromOption",
			Run: func(tracker *rewriter.Tracker) {
				callee, arguments := buildFromOptionReplacement(tracker, ctx.TypeParser, ctx.SourceFile, match)
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

func buildFromOptionReplacement(tracker *rewriter.Tracker, tp *typeparser.TypeParser, sf *ast.SourceFile, match rules.OptionMatchToFromOptionMatch) (*ast.Node, *ast.NodeList) {
	fromOption := tracker.NewPropertyAccessPath(typeparser.FindEffectModuleReference(sf).Append("fromOption")...)

	// Effect.fromOption fails with a new NoSuchElementError by default.
	if isDefaultNoSuchElementError(tp, match.FailureNode) {
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

func isDefaultNoSuchElementError(tp *typeparser.TypeParser, node *ast.Node) bool {
	node = ast.SkipParentheses(node)
	if node == nil || node.Kind != ast.KindNewExpression {
		return false
	}
	expression := node.AsNewExpression()
	if expression == nil || expression.Expression == nil ||
		expression.TypeArguments != nil && len(expression.TypeArguments.Nodes) > 0 ||
		expression.Arguments != nil && len(expression.Arguments.Nodes) > 0 {
		return false
	}
	return tp.IsNodeReferenceToEffectCauseModuleApi(expression.Expression, "NoSuchElementError")
}
