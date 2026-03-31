// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
)

func firstEffectFnFunctionArgument(args []*ast.Node) (*ast.Node, []*ast.Node) {
	for i, arg := range args {
		if arg == nil {
			continue
		}
		switch arg.Kind {
		case ast.KindArrowFunction, ast.KindFunctionExpression:
			if i+1 < len(args) {
				return arg, args[i+1:]
			}
			return arg, nil
		}
	}
	return nil, nil
}

func isGeneratorFunctionNode(node *ast.Node) bool {
	if node == nil || node.Kind != ast.KindFunctionExpression {
		return false
	}
	fn := node.AsFunctionExpression()
	return fn != nil && fn.AsteriskToken != nil
}

// EffectFnCall parses a node as Effect.fn(<regularFn>) or Effect.fn("name")(<regularFn>).
// It matches only non-generator variants (arrow function or function expression without asteriskToken).
// Returns nil when the node is not an Effect.fn non-generator call.
func (tp *TypeParser) EffectFnCall(node *ast.Node) *EffectFnCallResult {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	return Cached(&tp.links.EffectFnCall, node, func() *EffectFnCallResult {
		call := node.AsCallExpression()
		if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
			return nil
		}

		bodyArg, pipeArgs := firstEffectFnFunctionArgument(call.Arguments.Nodes)
		if bodyArg == nil || isGeneratorFunctionNode(bodyArg) {
			return nil
		}

		// Determine the expression to check for Effect.fn reference.
		// For curried calls like Effect.fn("name")(regularFn), call.Expression is a CallExpression.
		// For direct calls like Effect.fn(regularFn), call.Expression is a PropertyAccessExpression.
		expr := call.Expression
		if expr == nil {
			return nil
		}

		var expressionToCheck *ast.Node
		var traceExpression *ast.Node

		if expr.Kind == ast.KindCallExpression {
			innerCall := expr.AsCallExpression()
			if innerCall == nil || innerCall.Expression == nil {
				return nil
			}
			expressionToCheck = innerCall.Expression

			// Extract trace expression from curried form: Effect.fn("name")(...)
			if innerCall.Arguments != nil && len(innerCall.Arguments.Nodes) > 0 {
				traceExpression = innerCall.Arguments.Nodes[0]
			}
		} else {
			expressionToCheck = expr
		}

		if expressionToCheck == nil || expressionToCheck.Kind != ast.KindPropertyAccessExpression {
			return nil
		}

		if !tp.IsNodeReferenceToEffectModuleApi(expressionToCheck, "fn") {
			return nil
		}

		propertyAccess := expressionToCheck.AsPropertyAccessExpression()
		if propertyAccess == nil {
			return nil
		}

		return &EffectFnCallResult{
			Call:            call,
			Kind:            "fn",
			EffectModule:    propertyAccess.Expression,
			BodyFunction:    bodyArg,
			PipeArguments:   pipeArgs,
			TraceExpression: traceExpression,
		}
	})
}
