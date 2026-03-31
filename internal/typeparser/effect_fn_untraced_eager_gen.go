// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
)

// EffectFnUntracedEagerGenCall parses a node as Effect.fnUntracedEager(<generator>).
// It matches only generator-based variants (function with asteriskToken).
// Unlike EffectFnGenCall, it does not support curried calls.
// Returns nil when the node is not an Effect.fnUntracedEager generator call.
func (tp *TypeParser) EffectFnUntracedEagerGenCall(node *ast.Node) *EffectGenCallResult {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	return Cached(&tp.links.EffectFnUntracedEagerGenCall, node, func() *EffectGenCallResult {
		call := node.AsCallExpression()
		if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
			return nil
		}

		bodyArg, _ := firstEffectFnFunctionArgument(call.Arguments.Nodes)
		if !isGeneratorFunctionNode(bodyArg) {
			return nil
		}
		genFn := bodyArg.AsFunctionExpression()

		// fnUntracedEager only supports direct calls (no curried naming).
		// call.Expression must be a PropertyAccessExpression directly.
		expr := call.Expression
		if expr == nil || expr.Kind != ast.KindPropertyAccessExpression {
			return nil
		}

		if !tp.IsNodeReferenceToEffectModuleApi(expr, "fnUntracedEager") {
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
	})
}
