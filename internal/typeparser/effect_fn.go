// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/effect-ts/tsgo/internal/readonly"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

type EffectFnVariant string

const (
	EffectFnVariantFn              EffectFnVariant = "fn"
	EffectFnVariantFnUntraced      EffectFnVariant = "fnUntraced"
	EffectFnVariantFnUntracedEager EffectFnVariant = "fnUntracedEager"
)

// EffectFnCallResult represents a parsed Effect.fn-family call.
type EffectFnCallResult struct {
	call               *ast.CallExpression
	variant            EffectFnVariant
	effectModule       *ast.Expression
	optionsNode        *ast.Node
	functionNode       *ast.Node // ArrowFunction or FunctionExpression
	functionReturnType *checker.Type
	pipeArguments      []*ast.Node // Transformation args after the body (may be empty/nil)
	pipeArgsOutType    []*checker.Type
	traceExpression    *ast.Node // The name string from curried Effect.fn("name")(...), or nil
}

func (r EffectFnCallResult) Call() *ast.CallExpression { return r.call }

func (r EffectFnCallResult) Variant() EffectFnVariant { return r.variant }

func (r EffectFnCallResult) EffectModule() *ast.Expression { return r.effectModule }

func (r EffectFnCallResult) OptionsNode() *ast.Node { return r.optionsNode }

func (r EffectFnCallResult) FunctionNode() *ast.Node { return r.functionNode }

func (r EffectFnCallResult) FunctionReturnType() *checker.Type { return r.functionReturnType }

func (r EffectFnCallResult) PipeArguments() readonly.Slice[*ast.Node] {
	return readonly.New(r.pipeArguments)
}

func (r EffectFnCallResult) PipeArgsOutType() readonly.Slice[*checker.Type] {
	return readonly.New(r.pipeArgsOutType)
}

func (r EffectFnCallResult) TraceExpression() *ast.Node { return r.traceExpression }

func (r *EffectFnCallResult) IsGenerator() bool {
	return r.GeneratorFunction() != nil
}

func (r *EffectFnCallResult) GeneratorFunction() *ast.FunctionExpression {
	if r == nil || r.functionNode == nil || r.functionNode.Kind != ast.KindFunctionExpression {
		return nil
	}
	fn := r.functionNode.AsFunctionExpression()
	if fn == nil || fn.AsteriskToken == nil {
		return nil
	}
	return fn
}

func (r *EffectFnCallResult) Body() *ast.BlockOrExpression {
	if r == nil || r.functionNode == nil {
		return nil
	}
	switch r.functionNode.Kind {
	case ast.KindArrowFunction:
		fn := r.functionNode.AsArrowFunction()
		if fn == nil {
			return nil
		}
		return fn.Body
	case ast.KindFunctionExpression:
		fn := r.functionNode.AsFunctionExpression()
		if fn == nil {
			return nil
		}
		return fn.Body
	default:
		return nil
	}
}

func splitEffectFnArguments(args []*ast.Node) (*ast.Node, *ast.Node, []*ast.Node) {
	start := 0
	var options *ast.Node
	if len(args) > 0 {
		first := args[0]
		if first != nil && first.Kind != ast.KindArrowFunction && first.Kind != ast.KindFunctionExpression {
			options = first
			start = 1
		}
	}
	for i := start; i < len(args); i++ {
		arg := args[i]
		if arg == nil {
			continue
		}
		switch arg.Kind {
		case ast.KindArrowFunction, ast.KindFunctionExpression:
			if i+1 < len(args) {
				return options, arg, args[i+1:]
			}
			return options, arg, nil
		}
	}
	return options, nil, nil
}

func isGeneratorFunctionNode(node *ast.Node) bool {
	if node == nil || node.Kind != ast.KindFunctionExpression {
		return false
	}
	fn := node.AsFunctionExpression()
	return fn != nil && fn.AsteriskToken != nil
}

func (tp *TypeParser) buildEffectFnPipeArgsOutType(call *ast.CallExpression, trailingStartIndex int, pipeArgs []*ast.Node) []*checker.Type {
	outTypes := make([]*checker.Type, len(pipeArgs))
	if tp == nil || tp.checker == nil || call == nil {
		return outTypes
	}

	callNode := call.AsNode()
	for i := range pipeArgs {
		argIndex := trailingStartIndex + i
		contextualType := tp.checker.GetContextualTypeForArgumentAtIndex(callNode, argIndex)
		if contextualType == nil {
			continue
		}
		callSigs := tp.checker.GetSignaturesOfType(contextualType, checker.SignatureKindCall)
		if len(callSigs) == 0 {
			continue
		}
		outTypes[i] = tp.checker.GetReturnTypeOfSignature(callSigs[0])
	}

	return outTypes
}

func (tp *TypeParser) buildEffectFnFunctionReturnType(call *ast.CallExpression, trailingStartIndex int, pipeArgs []*ast.Node) *checker.Type {
	if tp == nil || tp.checker == nil || call == nil {
		return nil
	}

	if len(pipeArgs) == 0 {
		fnType := tp.GetTypeAtLocation(call.AsNode())
		if fnType == nil {
			return nil
		}
		callSigs := tp.checker.GetSignaturesOfType(fnType, checker.SignatureKindCall)
		if len(callSigs) == 0 {
			return nil
		}
		return tp.checker.GetReturnTypeOfSignature(callSigs[0])
	}

	resolved := tp.checker.GetResolvedSignature(call.AsNode())
	if resolved == nil {
		return nil
	}
	params := resolved.Parameters()
	if trailingStartIndex >= len(params) {
		return nil
	}
	firstPipeParamType := tp.checker.GetTypeOfSymbolAtLocation(params[trailingStartIndex], pipeArgs[0])
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

// EffectFnCall parses a node as an Effect.fn-family call.
// It supports fn, fnUntraced, and fnUntracedEager, both regular and generator forms.
func (tp *TypeParser) EffectFnCall(node *ast.Node) *EffectFnCallResult {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	return Cached(&tp.links.EffectFnCall, node, func() *EffectFnCallResult {
		call := node.AsCallExpression()
		if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
			return nil
		}

		optionsNode, bodyArg, pipeArgs := splitEffectFnArguments(call.Arguments.Nodes)
		if bodyArg == nil {
			return nil
		}
		trailingStartIndex := len(call.Arguments.Nodes) - len(pipeArgs)

		// Determine the expression to check for Effect.fn reference.
		// For curried calls like Effect.fn("name")(regularFn), call.Expression is a CallExpression.
		// For direct calls like Effect.fn(regularFn), call.Expression is a PropertyAccessExpression.
		expr := call.Expression
		if expr == nil {
			return nil
		}

		var expressionToCheck *ast.Node
		var traceExpression *ast.Node
		var variant EffectFnVariant

		if expr.Kind == ast.KindCallExpression {
			innerCall := expr.AsCallExpression()
			if innerCall == nil || innerCall.Expression == nil {
				return nil
			}
			expressionToCheck = innerCall.Expression

			// Extract trace expression from curried form: Effect.fn("name")(...)
			if innerCall.Arguments != nil && len(innerCall.Arguments.Nodes) > 0 {
				traceExpression = innerCall.Arguments.Nodes[0]
			}
		} else {
			expressionToCheck = expr
		}

		if expressionToCheck == nil {
			return nil
		}

		switch {
		case tp.IsNodeReferenceToEffectModuleApi(expressionToCheck, "fn"):
			variant = EffectFnVariantFn
		case tp.IsNodeReferenceToEffectModuleApi(expressionToCheck, "fnUntraced"):
			if traceExpression != nil {
				return nil
			}
			variant = EffectFnVariantFnUntraced
		case tp.IsNodeReferenceToEffectModuleApi(expressionToCheck, "fnUntracedEager"):
			if traceExpression != nil {
				return nil
			}
			variant = EffectFnVariantFnUntracedEager
		default:
			return nil
		}

		var effectModule *ast.Node
		if expressionToCheck.Kind == ast.KindPropertyAccessExpression {
			propertyAccess := expressionToCheck.AsPropertyAccessExpression()
			if propertyAccess != nil {
				effectModule = propertyAccess.Expression
			}
		}

		return &EffectFnCallResult{
			call:               call,
			variant:            variant,
			effectModule:       effectModule,
			optionsNode:        optionsNode,
			functionNode:       bodyArg,
			functionReturnType: tp.buildEffectFnFunctionReturnType(call, trailingStartIndex, pipeArgs),
			pipeArguments:      pipeArgs,
			pipeArgsOutType:    tp.buildEffectFnPipeArgsOutType(call, trailingStartIndex, pipeArgs),
			traceExpression:    traceExpression,
		}
	})
}
