// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// EffectGenCallResult represents a parsed Effect.gen(...) call.
type EffectGenCallResult struct {
	Call               *ast.CallExpression
	EffectModule       *ast.Expression
	OptionsNode        *ast.Node
	GeneratorFunction  *ast.FunctionExpression
	Body               *ast.BlockOrExpression
	FunctionReturnType *checker.Type
	PipeArguments      []*ast.Node
}

func (tp *TypeParser) buildEffectGenFunctionReturnType(call *ast.CallExpression, trailingStartIndex int, pipeArgs []*ast.Node) *checker.Type {
	if tp == nil || tp.checker == nil || call == nil {
		return nil
	}

	if len(pipeArgs) == 0 {
		return tp.GetTypeAtLocation(call.AsNode())
	}

	firstPipeParamType := tp.checker.GetContextualTypeForArgumentAtIndex(call.AsNode(), trailingStartIndex)
	if firstPipeParamType == nil {
		return nil
	}
	firstPipeCallSigs := tp.checker.GetSignaturesOfType(firstPipeParamType, checker.SignatureKindCall)
	if len(firstPipeCallSigs) == 0 {
		return nil
	}
	pipeInputParams := firstPipeCallSigs[0].Parameters()
	if len(pipeInputParams) == 0 {
		return nil
	}
	return tp.checker.GetTypeOfSymbolAtLocation(pipeInputParams[0], pipeArgs[0])
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
		trailingStartIndex := len(call.Arguments.Nodes) - len(pipeArgs)

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
			Call:               call,
			EffectModule:       propertyAccess.Expression,
			OptionsNode:        optionsNode,
			GeneratorFunction:  genFn,
			Body:               genFn.Body,
			FunctionReturnType: tp.buildEffectGenFunctionReturnType(call, trailingStartIndex, pipeArgs),
			PipeArguments:      pipeArgs,
		}
	})
}
