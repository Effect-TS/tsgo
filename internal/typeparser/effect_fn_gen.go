// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// EffectFnGenCall parses a node as Effect.fn(<generator>) or Effect.fn("name")(<generator>).
// It matches only generator-based variants (function with asteriskToken).
// Returns nil when the node is not an Effect.fn generator call.
func EffectFnGenCall(c *checker.Checker, node *ast.Node) *EffectGenCallResult {
	if c == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	links := GetEffectLinks(c)
	return Cached(&links.EffectFnGenCall, node, func() *EffectGenCallResult {
		call := node.AsCallExpression()
		if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
			return nil
		}

		// Scan arguments for the first FunctionExpression with asteriskToken
		var genFn *ast.FunctionExpression
		for _, arg := range call.Arguments.Nodes {
			if arg != nil && arg.Kind == ast.KindFunctionExpression {
				fn := arg.AsFunctionExpression()
				if fn != nil && fn.AsteriskToken != nil {
					genFn = fn
					break
				}
			}
		}
		if genFn == nil {
			return nil
		}

		// Determine the expression to check for Effect.fn reference.
		// For curried calls like Effect.fn("name")(function*(){}), call.Expression is a CallExpression.
		// For direct calls like Effect.fn(function*(){}), call.Expression is a PropertyAccessExpression.
		expr := call.Expression
		if expr == nil {
			return nil
		}

		var expressionToCheck *ast.Node
		if expr.Kind == ast.KindCallExpression {
			innerCall := expr.AsCallExpression()
			if innerCall == nil || innerCall.Expression == nil {
				return nil
			}
			expressionToCheck = innerCall.Expression
		} else {
			expressionToCheck = expr
		}

		if expressionToCheck == nil || expressionToCheck.Kind != ast.KindPropertyAccessExpression {
			return nil
		}

		if !IsNodeReferenceToEffectModuleApi(c, expressionToCheck, "fn") {
			return nil
		}

		propertyAccess := expressionToCheck.AsPropertyAccessExpression()
		if propertyAccess == nil {
			return nil
		}

		return &EffectGenCallResult{
			Call:              call,
			EffectModule:      propertyAccess.Expression,
			GeneratorFunction: genFn,
			Body:              genFn.Body,
		}
	})
}
