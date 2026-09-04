package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// UnwrapIdentityForwarder returns the callee of a one-argument function that
// forwards its argument unchanged, any explicit type arguments on the
// forwarded call, and the forwarder's parameter declaration so that callers
// can apply stricter checks (e.g. reject annotated parameters). All other
// expressions are returned as-is with no type arguments and no parameter.
//
// For example, both fn and value => fn(value) normalize to fn.
func (tp *TypeParser) UnwrapIdentityForwarder(node *ast.Node) (target *ast.Node, typeArguments *ast.NodeList, parameter *ast.Node) {
	if node == nil {
		return nil, nil, nil
	}

	node = ast.SkipParentheses(node)
	if tp == nil || tp.checker == nil {
		return node, nil, nil
	}
	lazy := ParseLazyExpression(node, LazyExpressionNone)
	if lazy == nil || len(lazy.Params) != 1 || lazy.Expression == nil {
		return node, nil, nil
	}

	parameterNode := lazy.Params[0]
	if parameterNode == nil || parameterNode.Kind != ast.KindParameter {
		return node, nil, nil
	}
	parameterDeclaration := parameterNode.AsParameterDeclaration()
	if parameterDeclaration == nil || parameterDeclaration.Name() == nil || parameterDeclaration.Name().Kind != ast.KindIdentifier ||
		parameterDeclaration.DotDotDotToken != nil || parameterDeclaration.Initializer != nil {
		return node, nil, nil
	}

	expression := ast.SkipParentheses(lazy.Expression)
	if expression == nil || expression.Kind != ast.KindCallExpression {
		return node, nil, nil
	}
	call := expression.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil || len(call.Arguments.Nodes) != 1 {
		return node, nil, nil
	}

	argument := ast.SkipParentheses(call.Arguments.Nodes[0])
	if argument == nil || argument.Kind != ast.KindIdentifier {
		return node, nil, nil
	}

	parameterSymbol := tp.GetSymbolAtLocation(parameterDeclaration.Name())
	argumentSymbol := tp.GetSymbolAtLocation(argument)
	if parameterSymbol == nil || argumentSymbol == nil ||
		checker.Checker_getSymbolIfSameReference(tp.checker, parameterSymbol, argumentSymbol) == nil {
		return node, nil, nil
	}

	return ast.SkipParentheses(call.Expression), call.TypeArguments, parameterNode
}
