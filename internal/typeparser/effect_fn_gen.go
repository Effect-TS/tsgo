// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
)

// EffectFnGenCall parses a node as Effect.fn(<generator>, ...)
// or Effect.fn("name")(<generator>, ...).
// It matches only generator-based variants (function with asteriskToken).
func (tp *TypeParser) EffectFnGenCall(node *ast.Node) *EffectFnGenCallResult {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	return Cached(&tp.links.EffectFnGenCall, node, func() *EffectFnGenCallResult {
		call := node.AsCallExpression()
		if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
			return nil
		}

		bodyArg, pipeArgs := firstEffectFnFunctionArgument(call.Arguments.Nodes)
		if !isGeneratorFunctionNode(bodyArg) {
			return nil
		}
		genFn := bodyArg.AsFunctionExpression()

		// Determine the expression to check for Effect.fn reference.
		// For curried calls like Effect.fn("name")(function*(){}), call.Expression is a CallExpression.
		// For direct calls like Effect.fn(function*(){}), call.Expression is a PropertyAccessExpression.
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

		return &EffectFnGenCallResult{
			Call:              call,
			EffectModule:      propertyAccess.Expression,
			GeneratorFunction: genFn,
			Body:              genFn.Body,
			Variant:           "fn",
			PipeArguments:     pipeArgs,
			TraceExpression:   traceExpression,
		}
	})
}
