// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
)

// EffectGenCall parses a node as Effect.gen(<generator>).
// Returns nil when the node is not an Effect.gen call.
func (tp *TypeParser) EffectGenCall(node *ast.Node) *EffectGenCallResult {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	links := tp.GetEffectLinks()
	return Cached(&links.EffectGenCall, node, func() *EffectGenCallResult {
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

		if !tp.IsNodeReferenceToEffectModuleApi(expr, "gen") {
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
