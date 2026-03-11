// Package typeparser provides Effect type detection and parsing utilities.
package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// EffectFnCall parses a node as Effect.fn(<regularFn>) or Effect.fn("name")(<regularFn>).
// It matches only non-generator variants (arrow function or function expression without asteriskToken).
// Returns nil when the node is not an Effect.fn non-generator call.
func EffectFnCall(c *checker.Checker, node *ast.Node) *EffectFnCallResult {
	if c == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}

	call := node.AsCallExpression()
	if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
		return nil
	}

	// Scan arguments for the first ArrowFunction or non-generator FunctionExpression
	var bodyArg *ast.Node
	var bodyIndex int
	for i, arg := range call.Arguments.Nodes {
		if arg == nil {
			continue
		}
		if arg.Kind == ast.KindArrowFunction {
			bodyArg = arg
			bodyIndex = i
			break
		}
		if arg.Kind == ast.KindFunctionExpression {
			fn := arg.AsFunctionExpression()
			if fn != nil && fn.AsteriskToken == nil {
				bodyArg = arg
				bodyIndex = i
				break
			}
		}
	}
	if bodyArg == nil {
		return nil
	}

	// Determine the expression to check for Effect.fn reference.
	// For curried calls like Effect.fn("name")(regularFn), call.Expression is a CallExpression.
	// For direct calls like Effect.fn(regularFn), call.Expression is a PropertyAccessExpression.
	expr := call.Expression
	if expr == nil {
		return nil
	}

	var expressionToCheck *ast.Node
	var traceExpression *ast.Node

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

	if expressionToCheck == nil || expressionToCheck.Kind != ast.KindPropertyAccessExpression {
		return nil
	}

	if !IsNodeReferenceToEffectModuleApi(c, expressionToCheck, "fn") {
		return nil
	}

	propertyAccess := expressionToCheck.AsPropertyAccessExpression()
	if propertyAccess == nil {
		return nil
	}

	// Extract pipe arguments (arguments after the body function)
	var pipeArgs []*ast.Node
	if bodyIndex+1 < len(call.Arguments.Nodes) {
		pipeArgs = call.Arguments.Nodes[bodyIndex+1:]
	}

	return &EffectFnCallResult{
		Call:            call,
		Kind:            "fn",
		EffectModule:    propertyAccess.Expression,
		BodyFunction:    bodyArg,
		PipeArguments:   pipeArgs,
		TraceExpression: traceExpression,
	}
}
