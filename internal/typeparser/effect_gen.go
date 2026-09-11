// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// EffectGenCallResult represents a parsed Effect.gen(...) call.
type EffectGenCallResult struct {
	Call              *ast.CallExpression
	EffectModule      *ast.Expression // Namespace receiver for Effect.gen; nil for named imports and local aliases.
	OptionsNode       *ast.Node
	GeneratorFunction *ast.FunctionExpression
	Body              *ast.BlockOrExpression
	PipeArguments     []*ast.Node
}

// EffectGenCall parses a node as Effect.gen(<generator>).
// Returns nil when the node is not an Effect.gen call.
func (tp *TypeParser) EffectGenCall(node *ast.Node) *EffectGenCallResult {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	return Cached(&tp.links.EffectGenCall, node, func() *EffectGenCallResult {
		call := node.AsCallExpression()
		if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
			return nil
		}

		optionsNode, bodyArg, pipeArgs := splitEffectFnArguments(call.Arguments.Nodes)
		if !isGeneratorFunctionNode(bodyArg) {
			return nil
		}
		genFn := bodyArg.AsFunctionExpression()

		expr := call.Expression
		if expr == nil || !tp.IsNodeReferenceToEffectModuleApi(expr, "gen") {
			return nil
		}

		var effectModule *ast.Node
		if expr.Kind == ast.KindPropertyAccessExpression {
			effectModule = expr.AsPropertyAccessExpression().Expression
		}

		return &EffectGenCallResult{
			Call:              call,
			EffectModule:      effectModule,
			OptionsNode:       optionsNode,
			GeneratorFunction: genFn,
			Body:              genFn.Body,
			PipeArguments:     pipeArgs,
		}
	})
}
