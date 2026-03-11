// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// EffectGenCall parses a node as Effect.gen(<generator>).
// Returns nil when the node is not an Effect.gen call.
func EffectGenCall(c *checker.Checker, node *ast.Node) *EffectGenCallResult {
	if c == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	call := node.AsCallExpression()
	if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
		return nil
	}

	// Scan arguments for the first FunctionExpression with asteriskToken.
	// The generator may not be the first argument when an options object
	// (e.g., {self: this}) is passed before it.
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

	expr := call.Expression
	if expr == nil || expr.Kind != ast.KindPropertyAccessExpression {
		return nil
	}

	propertyAccess := expr.AsPropertyAccessExpression()
	if propertyAccess == nil {
		return nil
	}

	if !IsNodeReferenceToEffectModuleApi(c, expr, "gen") {
		return nil
	}

	return &EffectGenCallResult{
		Call:              call,
		EffectModule:      propertyAccess.Expression,
		GeneratorFunction: genFn,
		Body:              genFn.Body,
	}
}
