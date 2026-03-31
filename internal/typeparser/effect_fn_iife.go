// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
)

// ParseEffectFnIife parses a node as an Effect.fn(...)() or Effect.fnUntraced(...)() IIFE.
// The node must be the outer call expression. Returns nil if no match.
func (tp *TypeParser) ParseEffectFnIife(node *ast.Node) *EffectFnIifeResult {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	return Cached(&tp.links.ParseEffectFnIife, node, func() *EffectFnIifeResult {
		outerCall := node.AsCallExpression()
		if outerCall == nil || outerCall.Expression == nil {
			return nil
		}

		// The callee of the outer call must itself be a call expression (double-call pattern)
		innerNode := outerCall.Expression
		if innerNode.Kind != ast.KindCallExpression {
			return nil
		}

		innerCall := innerNode.AsCallExpression()
		if innerCall == nil {
			return nil
		}

		if result := tp.EffectFnCall(innerNode); result != nil {
			genFn := result.GeneratorFunction()
			if genFn != nil {
				return &EffectFnIifeResult{
					OuterCall:         outerCall,
					InnerCall:         innerCall,
					EffectModule:      result.EffectModule,
					Variant:           string(result.Variant),
					GeneratorFunction: genFn,
					PipeArguments:     result.PipeArguments,
					TraceExpression:   result.TraceExpression,
				}
			}

			// Non-generator Effect.fn-family call
			return &EffectFnIifeResult{
				OuterCall:       outerCall,
				InnerCall:       innerCall,
				EffectModule:    result.EffectModule,
				Variant:         string(result.Variant),
				PipeArguments:   result.PipeArguments,
				TraceExpression: result.TraceExpression,
			}
		}

		return nil
	})
}
