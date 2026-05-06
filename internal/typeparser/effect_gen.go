// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
)

// EffectGenCallResult represents a parsed Effect.gen(...) call.
type EffectGenCallResult struct {
	Call              *ast.CallExpression
	EffectModule      *ast.Expression
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
			OptionsNode:       optionsNode,
			GeneratorFunction: genFn,
			Body:              genFn.Body,
			PipeArguments:     pipeArgs,
		}
	})
}
