// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// EffectFnUntracedGenCall parses a node as Effect.fnUntraced(<generator>).
// It matches only generator-based variants (function with asteriskToken).
// Unlike EffectFnGenCall, it does not support curried calls.
// Returns nil when the node is not an Effect.fnUntraced generator call.
func EffectFnUntracedGenCall(c *checker.Checker, node *ast.Node) *EffectGenCallResult {
	if c == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

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

	// fnUntraced only supports direct calls (no curried naming).
	// call.Expression must be a PropertyAccessExpression directly.
	expr := call.Expression
	if expr == nil || expr.Kind != ast.KindPropertyAccessExpression {
		return nil
	}

	if !IsNodeReferenceToEffectModuleApi(c, expr, "fnUntraced") {
		return nil
	}

	propertyAccess := expr.AsPropertyAccessExpression()
	if propertyAccess == nil {
		return nil
	}

	return &EffectGenCallResult{
		Call:              call,
		EffectModule:      propertyAccess.Expression,
		GeneratorFunction: genFn,
		Body:              genFn.Body,
	}
}
