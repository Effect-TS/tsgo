// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/effect-ts/tsgo/internal/readonly"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// EffectGenCallResult represents a parsed Effect.gen(...) call.
type EffectGenCallResult struct {
	call              *ast.CallExpression
	effectModule      *ast.Expression // Namespace receiver for Effect.gen; nil for named imports and local aliases.
	optionsNode       *ast.Node
	generatorFunction *ast.FunctionExpression
	body              *ast.BlockOrExpression
	pipeArguments     []*ast.Node
}

func (r EffectGenCallResult) Call() *ast.CallExpression { return r.call }

func (r EffectGenCallResult) EffectModule() *ast.Expression { return r.effectModule }

func (r EffectGenCallResult) OptionsNode() *ast.Node { return r.optionsNode }

func (r EffectGenCallResult) GeneratorFunction() *ast.FunctionExpression { return r.generatorFunction }

func (r EffectGenCallResult) Body() *ast.BlockOrExpression { return r.body }

func (r EffectGenCallResult) PipeArguments() readonly.Slice[*ast.Node] {
	return readonly.New(r.pipeArguments)
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
			call:              call,
			effectModule:      effectModule,
			optionsNode:       optionsNode,
			generatorFunction: genFn,
			body:              genFn.Body,
			pipeArguments:     pipeArgs,
		}
	})
}
