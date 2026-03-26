package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var UnnecessaryPipeChainFix = fixable.Fixable{
	Name:        "unnecessaryPipeChain",
	Description: "Rewrite as single pipe call",
	ErrorCodes:  []int32{tsdiag.Chained_pipe_calls_can_be_simplified_to_a_single_pipe_call_effect_unnecessaryPipeChain.Code()},
	FixIDs:      []string{"unnecessaryPipeChain_fix"},
	Run:         runUnnecessaryPipeChainFix,
}

func runUnnecessaryPipeChainFix(ctx *fixable.Context) []ls.CodeAction {

	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	sf := ctx.SourceFile

	matches := rules.AnalyzeUnnecessaryPipeChain(c, sf)

	var match *rules.UnnecessaryPipeChainMatch
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

	outer := match.Outer
	inner := match.Inner

	if action := ctx.NewFixAction(fixable.FixAction{
		Description: "Rewrite as single pipe call",
		Run: func(tracker *change.Tracker) {
			// Collect all arguments: deep-clone inner args then outer args
			allArgs := make([]*ast.Node, 0, len(inner.Args)+len(outer.Args))
			for _, arg := range inner.Args {
				allArgs = append(allArgs, tracker.DeepCloneNode(arg))
			}
			for _, arg := range outer.Args {
				allArgs = append(allArgs, tracker.DeepCloneNode(arg))
			}

			var replacementNode *ast.Node
			switch inner.Kind {
			case typeparser.TransformationKindPipe:
				// pipe(subject, f1, f2) + pipe(..., f3) => pipe(subject, f1, f2, f3)
				// Deep-clone the original callee (bare `pipe` or `Function.pipe`)
				clonedCallee := tracker.DeepCloneNode(inner.Node.AsCallExpression().Expression)
				// Build args: subject first, then all pipe args
				callArgs := make([]*ast.Node, 0, 1+len(allArgs))
				callArgs = append(callArgs, tracker.DeepCloneNode(inner.Subject))
				callArgs = append(callArgs, allArgs...)
				replacementNode = tracker.NewCallExpression(clonedCallee, nil, nil, tracker.NewNodeList(callArgs), ast.NodeFlagsNone)
			case typeparser.TransformationKindPipeable:
				// subject.pipe(f1, f2).pipe(f3) => subject.pipe(f1, f2, f3)
				clonedSubject := tracker.DeepCloneNode(inner.Subject)
				pipeAccess := tracker.NewPropertyAccessExpression(clonedSubject, nil, tracker.NewIdentifier("pipe"), ast.NodeFlagsNone)
				replacementNode = tracker.NewCallExpression(pipeAccess, nil, nil, tracker.NewNodeList(allArgs), ast.NodeFlagsNone)
			default:
				return
			}

			ast.SetParentInChildren(replacementNode)
			tracker.ReplaceNode(sf, outer.Node.AsNode(), replacementNode, nil)
		},
	}); action != nil {
		return []ls.CodeAction{*action}
	}
	return nil
}
