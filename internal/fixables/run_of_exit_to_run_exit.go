package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var RunOfExitToRunExitFix = fixable.Fixable{
	Name:        "runOfExitToRunExit",
	Description: "Replace with the dedicated Effect Exit runner",
	ErrorCodes: []int32{
		tsdiag.Effect_0_of_Effect_exit_re_implements_Effect_1_Use_the_dedicated_Exit_runner_directly_effect_runOfExitToRunExit.Code(),
	},
	FixIDs: []string{"runOfExitToRunExit_fix"},
	Run:    runRunOfExitToRunExitFix,
}

func runRunOfExitToRunExitFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeRunOfExitToRunExit(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		// A property access lets us preserve the existing Effect namespace alias.
		// Named imports still receive the diagnostic, but need import management
		// that this local rewrite deliberately does not attempt.
		if match.RunnerNameNode == nil || match.ExitTransformation == nil {
			return nil
		}

		description := "Replace with Effect." + match.ReplacementName
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: description,
			Run: func(tracker *rewriter.Tracker) {
				if !removeExitTransformation(tracker, ctx.SourceFile, match.ExitTransformation) {
					return
				}
				tracker.ReplaceNode(ctx.SourceFile, match.RunnerNameNode, tracker.NewIdentifier(match.ReplacementName), nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}

func removeExitTransformation(tracker *rewriter.Tracker, sf *ast.SourceFile, transformation *typeparser.PipingFlowTransformation) bool {
	if tracker == nil || sf == nil || transformation == nil || transformation.Callee == nil {
		return false
	}

	switch transformation.Kind {
	case typeparser.TransformationKindDataFirst, typeparser.TransformationKindDataLast, typeparser.TransformationKindCall:
		callNode, call := directCallForCallee(transformation.Callee)
		if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) != 1 {
			return false
		}
		tracker.ReplaceNode(sf, callNode, tracker.DeepCloneNode(call.Arguments.Nodes[0]), nil)
		return true

	case typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable:
		callNode, call, argumentIndex := containingCallArgumentForExit(transformation.Callee)
		if call == nil || call.Arguments == nil || argumentIndex < 0 {
			return false
		}
		arguments := call.Arguments.Nodes
		minimumArguments := 1
		if transformation.Kind == typeparser.TransformationKindPipe {
			minimumArguments = 2 // Function.pipe also contains its subject.
		}
		if len(arguments) == minimumArguments {
			var subject *ast.Node
			if transformation.Kind == typeparser.TransformationKindPipe {
				subject = arguments[0]
			} else if call.Expression != nil && call.Expression.Kind == ast.KindPropertyAccessExpression {
				subject = call.Expression.AsPropertyAccessExpression().Expression
			}
			if subject == nil {
				return false
			}
			tracker.ReplaceNode(sf, callNode, tracker.DeepCloneNode(subject), nil)
			return true
		}

		if argumentIndex < len(arguments)-1 {
			start := scanner.GetTokenPosOfNode(arguments[argumentIndex], sf, false)
			end := scanner.GetTokenPosOfNode(arguments[argumentIndex+1], sf, false)
			tracker.DeleteRange(sf, core.NewTextRange(start, end))
			return true
		}
		if argumentIndex > 0 {
			tracker.DeleteRange(sf, core.NewTextRange(arguments[argumentIndex-1].End(), arguments[argumentIndex].End()))
			return true
		}
	}

	return false
}

func directCallForCallee(callee *ast.Node) (*ast.Node, *ast.CallExpression) {
	if callee == nil || callee.Parent == nil || callee.Parent.Kind != ast.KindCallExpression {
		return nil, nil
	}
	call := callee.Parent.AsCallExpression()
	if call == nil || call.Expression != callee {
		return nil, nil
	}
	return callee.Parent, call
}

func containingCallArgumentForExit(node *ast.Node) (*ast.Node, *ast.CallExpression, int) {
	for child := node; child != nil && child.Parent != nil; child = child.Parent {
		parent := child.Parent
		if parent.Kind != ast.KindCallExpression {
			continue
		}
		call := parent.AsCallExpression()
		if call == nil || call.Arguments == nil {
			continue
		}
		for index, argument := range call.Arguments.Nodes {
			if argument == child {
				return parent, call, index
			}
		}
	}
	return nil, nil, -1
}
