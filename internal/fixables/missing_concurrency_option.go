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

var MissingConcurrencyOptionFix = fixable.Fixable{
	Name:        "missingConcurrencyOption",
	Description: "Add `concurrency: 1` to keep the sequential behaviour explicit",
	ErrorCodes: []int32{
		tsdiag.Effect_0_runs_sequentially_unless_concurrency_is_set_pass_concurrency_Colon_1_to_keep_sequential_execution_or_set_the_concurrency_you_intend_effect_missingConcurrencyOption.Code(),
	},
	FixIDs: []string{"missingConcurrencyOption_fix"},
	Run:    runMissingConcurrencyOptionFix,
}

func runMissingConcurrencyOptionFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	for _, match := range rules.AnalyzeMissingConcurrencyOption(ctx.TypeParser, ctx.Checker, sf) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.OptionsUnderSatisfies {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Add `concurrency: 1`",
			Run: func(tracker *rewriter.Tracker) {
				addSequentialConcurrency(tracker, sf, match)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

func newSequentialConcurrencyProperty(tracker *rewriter.Tracker) *ast.Node {
	return tracker.NewPropertyAssignment(nil, tracker.NewIdentifier("concurrency"), nil, nil, tracker.NewNumericLiteral("1", ast.TokenFlagsNone))
}

func newSequentialConcurrencyOptions(tracker *rewriter.Tracker) *ast.Node {
	return tracker.NewObjectLiteralExpression(tracker.NewNodeList([]*ast.Node{newSequentialConcurrencyProperty(tracker)}), false)
}

func addSequentialConcurrency(tracker *rewriter.Tracker, sf *ast.SourceFile, match rules.MissingConcurrencyOptionMatch) {
	if match.OptionsLiteral != nil {
		addSequentialConcurrencyToLiteral(tracker, sf, match.OptionsLiteral)
		return
	}

	step := match.Flow.Transformations().At(match.StepIndex)
	switch {
	case rules.IsInputOnlyCall(step):
		input := match.Flow.TransformationInputNode(match.StepIndex)
		if input == nil {
			return
		}
		replacement := tracker.NewCallExpression(
			tracker.DeepCloneNode(step.Callee()),
			nil,
			cloneTypeArguments(tracker, step.TypeArguments()),
			tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(input), newSequentialConcurrencyOptions(tracker)}),
			ast.NodeFlagsNone,
		)
		tracker.ReplacePipingFlowPrefix(sf, match.Flow, match.StepIndex+1, replacement)
	case step.Kind() == typeparser.TransformationKindDataFirst,
		step.Kind() == typeparser.TransformationKindPipe,
		step.Kind() == typeparser.TransformationKindPipeable,
		step.Kind() == typeparser.TransformationKindCall:
		arguments := make([]*ast.Node, 0, step.Args().Len()+1)
		for _, argument := range step.Args().All() {
			arguments = append(arguments, tracker.DeepCloneNode(argument))
		}
		arguments = append(arguments, newSequentialConcurrencyOptions(tracker))
		tracker.ReplacePipingFlowTransformation(sf, &step, rewriter.PipingFlowTransformationReplacement{
			Callee:        tracker.DeepCloneNode(step.Callee()),
			TypeArguments: cloneTypeArguments(tracker, step.TypeArguments()),
			Arguments:     tracker.NewNodeList(arguments),
		})
	}
}

func addSequentialConcurrencyToLiteral(tracker *rewriter.Tracker, sf *ast.SourceFile, literal *ast.Node) {
	properties := literal.AsObjectLiteralExpression().Properties
	if properties == nil || len(properties.Nodes) == 0 {
		replacement := newSequentialConcurrencyOptions(tracker)
		ast.SetParentInChildren(replacement)
		tracker.ReplaceNode(sf, literal, replacement, nil)
		return
	}
	tracker.InsertNodeInListAfter(sf, properties.Nodes[len(properties.Nodes)-1], newSequentialConcurrencyProperty(tracker), properties)
}

func cloneTypeArguments(tracker *rewriter.Tracker, typeArguments *ast.NodeList) *ast.NodeList {
	if typeArguments == nil {
		return nil
	}
	nodes := make([]*ast.Node, len(typeArguments.Nodes))
	for i, node := range typeArguments.Nodes {
		nodes[i] = tracker.DeepCloneNode(node)
	}
	return tracker.NewNodeList(nodes)
}
