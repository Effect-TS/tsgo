package fixables

import (
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// removePipingFlowTransformation removes one step from its piping flow and
// leaves the step's input in place. A pipe argument is deleted, a pipe call left
// without steps collapses to its subject, and a data-first, data-last or unary
// application is replaced by its subject. It edits nothing and returns false
// when the step's shape is not recognized.
func removePipingFlowTransformation(tracker *rewriter.Tracker, sf *ast.SourceFile, transformation *typeparser.PipingFlowTransformation) bool {
	if tracker == nil || sf == nil || transformation == nil || transformation.Callee() == nil {
		return false
	}

	switch transformation.Kind() {
	case typeparser.TransformationKindDataFirst, typeparser.TransformationKindDataLast, typeparser.TransformationKindCall:
		callNode, call := directCallForCallee(transformation.Callee())
		if transformation.Kind() == typeparser.TransformationKindCall && transformation.Args().Len() > 0 {
			// A curried step `factory(args)(subject)` is applied by the outer call.
			callNode, call = directCallForCallee(callNode)
		}
		if call == nil || call.Arguments == nil {
			return false
		}
		arguments := call.Arguments.Nodes
		var subject *ast.Node
		switch transformation.Kind() {
		case typeparser.TransformationKindDataFirst:
			if len(arguments) == transformation.Args().Len()+1 {
				subject = arguments[0]
			}
		case typeparser.TransformationKindDataLast:
			if len(arguments) == transformation.Args().Len()+1 {
				subject = arguments[len(arguments)-1]
			}
		default:
			if len(arguments) == 1 {
				subject = arguments[0]
			}
		}
		if subject == nil {
			return false
		}
		tracker.ReplaceNode(sf, callNode, tracker.DeepCloneNode(subject), nil)
		return true

	case typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable,
		typeparser.TransformationKindEffectFn, typeparser.TransformationKindEffectFnUntraced:
		callNode, call, argumentIndex := containingCallArgument(transformation.Callee())
		if call == nil || call.Arguments == nil || argumentIndex < 0 {
			return false
		}
		arguments := call.Arguments.Nodes
		// Effect.fn pipelines always keep their body argument, so they never collapse.
		minimumArguments := 0
		switch transformation.Kind() {
		case typeparser.TransformationKindPipeable:
			minimumArguments = 1
		case typeparser.TransformationKindPipe:
			minimumArguments = 2 // Function.pipe also contains its subject.
		}
		if len(arguments) == minimumArguments {
			var subject *ast.Node
			if transformation.Kind() == typeparser.TransformationKindPipe {
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

func containingCallArgument(node *ast.Node) (*ast.Node, *ast.CallExpression, int) {
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
