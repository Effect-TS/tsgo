package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
)

// ParsedLazyExpression represents a parsed arrow function or function expression
// with its inner expression extracted.
type ParsedLazyExpression struct {
	Node       *ast.Node   // The original ArrowFunction or FunctionExpression node
	Params     []*ast.Node // Parameter declarations (empty when parsed with thunk=true)
	Body       *ast.Node   // The function body as written (Expression or Block)
	Expression *ast.Node   // The inner expression (return value)
}

// ParseLazyExpression parses an arrow function or function expression, extracting its inner expression.
// When thunk is true, the function must have zero parameters.
// Returns nil if the node is not a valid lazy expression.
func ParseLazyExpression(node *ast.Node, thunk bool) *ParsedLazyExpression {
	if node == nil {
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

	// When thunk=true, reject functions with any parameters
	if thunk && params != nil && len(params.Nodes) > 0 {
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
		Node:       node,
		Params:     paramNodes,
		Body:       body,
		Expression: expr,
	}
}
