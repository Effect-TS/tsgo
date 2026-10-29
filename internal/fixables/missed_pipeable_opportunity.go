package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/readonly"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var MissedPipeableOpportunityFix = fixable.Fixable{
	Name:        "missedPipeableOpportunity",
	Description: "Convert to pipe style",
	ErrorCodes:  []int32{tsdiag.This_nested_call_structure_has_a_pipeable_form_0_pipe_represents_the_same_call_sequence_in_pipe_style_and_may_be_easier_to_read_effect_missedPipeableOpportunity.Code()},
	FixIDs:      []string{"missedPipeableOpportunity_fix"},
	Run:         runMissedPipeableOpportunityFix,
}

func runMissedPipeableOpportunityFix(ctx *fixable.Context) []ls.CodeAction {
	c := ctx.Checker

	sf := ctx.SourceFile

	minArgCount := 2
	effectConfig := ctx.Options
	if effectConfig != nil {
		minArgCount = effectConfig.GetPipeableMinArgCount()
	}

	matches := rules.AnalyzeMissedPipeableOpportunity(ctx.TypeParser, c, sf, minArgCount)

	var match *rules.MissedPipeableOpportunityMatch
	for i := range matches {
		diagRange := matches[i].Location
		if diagRange.Intersects(ctx.Span) || ctx.Span.ContainedBy(diagRange) {
			match = &matches[i]
			break
		}
	}
	if match == nil {
		return nil
	}

	if action := ctx.NewFixAction(fixable.FixAction{
		Description: "Convert to pipe style",
		Run: func(tracker *rewriter.Tracker) {
			buildMissedPipeableReplacement(tracker, sf, match)
		},
	}); action != nil {
		return []ls.CodeAction{*action}
	}
	return nil
}

// buildMissedPipeableReplacement builds the .pipe() replacement and applies it via tracker.ReplaceNode.
func buildMissedPipeableReplacement(tracker *rewriter.Tracker, sf *ast.SourceFile, match *rules.MissedPipeableOpportunityMatch) {
	flow := match.Flow

	subjectInput := flow.TransformationInputNode(match.PipeableStartIndex)
	if subjectInput == nil {
		return
	}
	subjectNode := tracker.DeepCloneNode(subjectInput)

	// Build pipe arguments from transformations
	pipeArgs := make([]*ast.Node, 0, len(match.PipeableTransformations))
	for _, t := range match.PipeableTransformations {
		var arg *ast.Node
		if t.Args().Len() > 0 {
			// It's a function call like Effect.map((x) => x + 1) -> create callee(args...)
			clonedCallee := tracker.DeepCloneNode(t.Callee())
			clonedArgs := make([]*ast.Node, t.Args().Len())
			for j, a := range t.Args().All() {
				clonedArgs[j] = tracker.DeepCloneNode(a)
			}
			arg = tracker.NewCallExpression(clonedCallee, nil, nil, tracker.NewNodeList(clonedArgs), ast.NodeFlagsNone)
		} else {
			// It's a constant like Effect.asVoid
			arg = tracker.DeepCloneNode(t.Callee())
		}
		pipeArgs = append(pipeArgs, arg)
	}

	// Create subject.pipe(arg1, arg2, ...)
	pipeAccess := tracker.NewPropertyAccessExpression(subjectNode, nil, tracker.NewIdentifier("pipe"), ast.NodeFlagsNone)
	pipeCall := tracker.NewCallExpression(pipeAccess, nil, nil, tracker.NewNodeList(pipeArgs), ast.NodeFlagsNone)

	// Wrap with "after" transformations if any
	var replacementNode *ast.Node
	if match.AfterTransformations.Len() > 0 {
		replacementNode = wrapWithAfterTransformations(tracker, pipeCall, match.AfterTransformations)
	} else {
		replacementNode = pipeCall
	}

	ast.SetParentInChildren(replacementNode)
	tracker.ReplaceNode(sf, match.Node, replacementNode, nil)
}

// wrapWithAfterTransformations wraps an inner node with the "after" transformations.
// For "call" kind: callee(innerNode)
// For "callWithOptionals" kind: callee(innerNode, args...)
// For "pipe"/"pipeable" kind with args: callee(args...)(innerNode) (curried)
// For "pipe"/"pipeable" kind without args: callee(innerNode)
func wrapWithAfterTransformations(tracker *rewriter.Tracker, inner *ast.Node, transformations readonly.Slice[typeparser.PipingFlowTransformation]) *ast.Node {
	result := inner
	for _, t := range transformations.All() {
		if t.Kind() == typeparser.TransformationKindEffectFn || t.Kind() == typeparser.TransformationKindEffectFnUntraced {
			continue
		}

		clonedCallee := tracker.DeepCloneNode(t.Callee())

		switch t.Kind() {
		case typeparser.TransformationKindCall:
			// Single-arg call: callee(result)
			result = tracker.NewCallExpression(clonedCallee, nil, nil, tracker.NewNodeList([]*ast.Node{result}), ast.NodeFlagsNone)
		case typeparser.TransformationKindCallWithOptionals:
			callArgs := []*ast.Node{result}
			for _, a := range t.Args().All() {
				callArgs = append(callArgs, tracker.DeepCloneNode(a))
			}
			result = tracker.NewCallExpression(clonedCallee, nil, nil, tracker.NewNodeList(callArgs), ast.NodeFlagsNone)
		default:
			// Pipe or pipeable transformation
			if t.Args().Len() > 0 {
				// Curried form: callee(args...)(result)
				clonedArgs := make([]*ast.Node, t.Args().Len())
				for j, a := range t.Args().All() {
					clonedArgs[j] = tracker.DeepCloneNode(a)
				}
				curriedCall := tracker.NewCallExpression(clonedCallee, nil, nil, tracker.NewNodeList(clonedArgs), ast.NodeFlagsNone)
				result = tracker.NewCallExpression(curriedCall, nil, nil, tracker.NewNodeList([]*ast.Node{result}), ast.NodeFlagsNone)
			} else {
				// Constant: callee(result)
				result = tracker.NewCallExpression(clonedCallee, nil, nil, tracker.NewNodeList([]*ast.Node{result}), ast.NodeFlagsNone)
			}
		}
	}
	return result
}
