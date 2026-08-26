package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// UnwrapIdentityForwarder returns the callee of a one-argument function that
// forwards its argument unchanged. All other expressions are returned as-is.
//
// For example, both fn and value => fn(value) normalize to fn.
func (tp *TypeParser) UnwrapIdentityForwarder(node *ast.Node) *ast.Node {
	if node == nil {
		return nil
	}

	node = ast.SkipParentheses(node)
	if tp == nil || tp.checker == nil {
		return node
	}
	if ast.GetCombinedModifierFlags(node)&ast.ModifierFlagsAsync != 0 {
		return node
	}
	if node.Kind == ast.KindFunctionExpression {
		function := node.AsFunctionExpression()
		if function != nil && function.AsteriskToken != nil {
			return node
		}
	}

	lazy := ParseLazyExpression(node, false)
	if lazy == nil || len(lazy.Params) != 1 || lazy.Expression == nil {
		return node
	}

	parameterNode := lazy.Params[0]
	if parameterNode == nil || parameterNode.Kind != ast.KindParameter {
		return node
	}
	parameter := parameterNode.AsParameterDeclaration()
	if parameter == nil || parameter.Name() == nil || parameter.Name().Kind != ast.KindIdentifier ||
		parameter.DotDotDotToken != nil || parameter.Initializer != nil {
		return node
	}

	expression := ast.SkipParentheses(lazy.Expression)
	if expression == nil || expression.Kind != ast.KindCallExpression {
		return node
	}
	call := expression.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil || len(call.Arguments.Nodes) != 1 {
		return node
	}

	argument := ast.SkipParentheses(call.Arguments.Nodes[0])
	if argument == nil || argument.Kind != ast.KindIdentifier {
		return node
	}

	parameterSymbol := tp.GetSymbolAtLocation(parameter.Name())
	argumentSymbol := tp.GetSymbolAtLocation(argument)
	if parameterSymbol == nil || argumentSymbol == nil ||
		checker.Checker_getSymbolIfSameReference(tp.checker, parameterSymbol, argumentSymbol) == nil {
		return node
	}

	return ast.SkipParentheses(call.Expression)
}
