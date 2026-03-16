// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// ParseEffectFnIife parses a node as an Effect.fn(...)() or Effect.fnUntraced(...)() IIFE.
// The node must be the outer call expression. Returns nil if no match.
func ParseEffectFnIife(c *checker.Checker, node *ast.Node) *EffectFnIifeResult {
	if c == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	links := GetEffectLinks(c)
	return Cached(&links.ParseEffectFnIife, node, func() *EffectFnIifeResult {
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

		// Try generator parsers first (priority order per spec)
		// a. Effect.fn generator
		if result := EffectFnGenCall(c, innerNode); result != nil {
			pipeArgs, traceExpr := extractGenCallExtras(innerCall, result.GeneratorFunction)
			return &EffectFnIifeResult{
				OuterCall:         outerCall,
				InnerCall:         innerCall,
				EffectModule:      result.EffectModule,
				Variant:           "fn",
				GeneratorFunction: result.GeneratorFunction,
				PipeArguments:     pipeArgs,
				TraceExpression:   traceExpr,
			}
		}

		// b. Effect.fnUntraced generator
		if result := EffectFnUntracedGenCall(c, innerNode); result != nil {
			pipeArgs, _ := extractGenCallExtras(innerCall, result.GeneratorFunction)
			return &EffectFnIifeResult{
				OuterCall:         outerCall,
				InnerCall:         innerCall,
				EffectModule:      result.EffectModule,
				Variant:           "fnUntraced",
				GeneratorFunction: result.GeneratorFunction,
				PipeArguments:     pipeArgs,
				TraceExpression:   nil, // fnUntraced has no curried form
			}
		}

		// c. Effect.fnUntracedEager generator
		if result := EffectFnUntracedEagerGenCall(c, innerNode); result != nil {
			pipeArgs, _ := extractGenCallExtras(innerCall, result.GeneratorFunction)
			return &EffectFnIifeResult{
				OuterCall:         outerCall,
				InnerCall:         innerCall,
				EffectModule:      result.EffectModule,
				Variant:           "fnUntracedEager",
				GeneratorFunction: result.GeneratorFunction,
				PipeArguments:     pipeArgs,
				TraceExpression:   nil, // fnUntracedEager has no curried form
			}
		}

		// d. Effect.fn non-generator
		if result := EffectFnCall(c, innerNode); result != nil {
			return &EffectFnIifeResult{
				OuterCall:       outerCall,
				InnerCall:       innerCall,
				EffectModule:    result.EffectModule,
				Variant:         result.Kind,
				PipeArguments:   result.PipeArguments,
				TraceExpression: result.TraceExpression,
			}
		}

		return nil
	})
}

// extractGenCallExtras extracts pipe arguments and trace expression from the raw AST
// of a generator-based Effect.fn call. The generator parsers (EffectFnGenCall, etc.)
// return EffectGenCallResult which lacks these fields, so we extract them manually.
func extractGenCallExtras(innerCall *ast.CallExpression, genFn *ast.FunctionExpression) ([]*ast.Node, *ast.Node) {
	var pipeArgs []*ast.Node
	var traceExpr *ast.Node

	// Find the generator function index in arguments and take everything after it as pipe args
	if innerCall.Arguments != nil {
		genFnNode := genFn.AsNode()
		for i, arg := range innerCall.Arguments.Nodes {
			if arg == genFnNode {
				if i+1 < len(innerCall.Arguments.Nodes) {
					pipeArgs = innerCall.Arguments.Nodes[i+1:]
				}
				break
			}
		}
	}

	// Check for curried form: if innerCall.Expression is a CallExpression,
	// it's Effect.fn("name")(...), and the first arg of that call is the trace expression
	if innerCall.Expression != nil && innerCall.Expression.Kind == ast.KindCallExpression {
		curried := innerCall.Expression.AsCallExpression()
		if curried != nil && curried.Arguments != nil && len(curried.Arguments.Nodes) > 0 {
			traceExpr = curried.Arguments.Nodes[0]
		}
	}

	return pipeArgs, traceExpr
}
