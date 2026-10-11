package typeparser

import (
	"github.com/effect-ts/tsgo/internal/readonly"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// ParsedLazyExpression represents a parsed arrow function or function expression
// with its inner expression extracted.
type ParsedLazyExpression struct {
	node       *ast.Node   // The original ArrowFunction or FunctionExpression node
	params     []*ast.Node // Parameter declarations (empty when parsed with thunk=true)
	body       *ast.Node   // The function body as written (Expression or Block)
	expression *ast.Node   // The inner expression (return value)
}

func (r ParsedLazyExpression) Node() *ast.Node { return r.node }

func (r ParsedLazyExpression) Params() readonly.Slice[*ast.Node] { return readonly.New(r.params) }

func (r ParsedLazyExpression) Body() *ast.Node { return r.body }

func (r ParsedLazyExpression) Expression() *ast.Node { return r.expression }

// LazyExpressionFlags controls which function shapes ParseLazyExpression accepts.
type LazyExpressionFlags uint8

const (
	LazyExpressionNone LazyExpressionFlags = 0
	// LazyExpressionThunk requires a function with no parameters.
	LazyExpressionThunk LazyExpressionFlags = 1 << iota
	// LazyExpressionAllowAsync additionally accepts async functions.
	LazyExpressionAllowAsync
	// LazyExpressionAllowGenerator additionally accepts generator functions.
	LazyExpressionAllowGenerator
)

// ParseLazyExpression parses an arrow function or function expression, extracting its inner expression.
// By default, only synchronous, non-generator functions are accepted.
// Returns nil if the node is not a valid lazy expression.
func ParseLazyExpression(node *ast.Node, flags LazyExpressionFlags) *ParsedLazyExpression {
	node = ast.SkipParentheses(node)
	if node == nil {
		return nil
	}
	if flags&LazyExpressionAllowAsync == 0 && ast.GetCombinedModifierFlags(node)&ast.ModifierFlagsAsync != 0 {
		return nil
	}

	var typeParams *ast.NodeList
	var params *ast.NodeList
	var body *ast.Node

	switch node.Kind {
	case ast.KindArrowFunction:
		fn := node.AsArrowFunction()
		typeParams = fn.TypeParameters
		params = fn.Parameters
		body = fn.Body
	case ast.KindFunctionExpression:
		fn := node.AsFunctionExpression()
		if fn.AsteriskToken != nil && flags&LazyExpressionAllowGenerator == 0 {
			return nil
		}
		typeParams = fn.TypeParameters
		params = fn.Parameters
		body = fn.Body
	default:
		return nil
	}

	// Reject functions with type parameters
	if typeParams != nil && len(typeParams.Nodes) > 0 {
		return nil
	}

	// Thunks must not declare parameters.
	if flags&LazyExpressionThunk != 0 && params != nil && len(params.Nodes) > 0 {
		return nil
	}

	if body == nil {
		return nil
	}

	// Build params list
	var paramNodes []*ast.Node
	if params != nil {
		paramNodes = params.Nodes
	}

	// Extract the inner expression
	var expr *ast.Node
	if body.Kind == ast.KindBlock {
		block := body.AsBlock()
		if block.Statements == nil || len(block.Statements.Nodes) != 1 {
			return nil
		}
		stmt := block.Statements.Nodes[0]
		if stmt.Kind != ast.KindReturnStatement {
			return nil
		}
		returnExpr := stmt.AsReturnStatement().Expression
		if returnExpr == nil {
			return nil
		}
		expr = returnExpr
	} else {
		// Expression body (arrow function only)
		expr = body
	}

	return &ParsedLazyExpression{
		node:       node,
		params:     paramNodes,
		body:       body,
		expression: expr,
	}
}
