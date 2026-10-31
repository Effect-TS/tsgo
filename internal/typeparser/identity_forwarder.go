package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// UnwrapIdentityForwarder returns the callee of a one-parameter function
// whose body is a piping flow that applies a single operator to that
// parameter, any explicit type arguments on that operator, and the
// forwarder's parameter declaration so that callers can apply stricter checks
// (e.g. reject annotated parameters). All other expressions are returned
// as-is with no type arguments and no parameter.
//
// For example, fn, value => fn(value), value => pipe(value, fn) and
// value => value.pipe(fn) all normalize to fn.
func (tp *TypeParser) UnwrapIdentityForwarder(node *ast.Node) (target *ast.Node, typeArguments *ast.NodeList, parameter *ast.Node) {
	if node == nil {
		return nil, nil, nil
	}

	node = ast.SkipParentheses(node)
	if tp == nil || tp.checker == nil {
		return node, nil, nil
	}
	lazy := ParseLazyExpression(node, LazyExpressionNone)
	if lazy == nil || len(lazy.params) != 1 || lazy.expression == nil {
		return node, nil, nil
	}

	parameterNode := lazy.params[0]
	if parameterNode == nil || parameterNode.Kind != ast.KindParameter {
		return node, nil, nil
	}
	parameterDeclaration := parameterNode.AsParameterDeclaration()
	if parameterDeclaration == nil || parameterDeclaration.Name() == nil || parameterDeclaration.Name().Kind != ast.KindIdentifier ||
		parameterDeclaration.DotDotDotToken != nil || parameterDeclaration.Initializer != nil {
		return node, nil, nil
	}
	parameterSymbol := tp.GetSymbolAtLocation(parameterDeclaration.Name())
	if parameterSymbol == nil {
		return node, nil, nil
	}

	isParameter := func(subject PipingFlowSubject) bool {
		argument := ast.SkipParentheses(subject.Node())
		if argument == nil || argument.Kind != ast.KindIdentifier {
			return false
		}
		argumentSymbol := tp.GetSymbolAtLocation(argument)
		return argumentSymbol != nil && checker.Checker_getSymbolIfSameReference(tp.checker, parameterSymbol, argumentSymbol) != nil
	}
	isBareOperator := func(transformation PipingFlowTransformation) bool {
		return transformation.Callee() != nil && transformation.Args().Len() == 0
	}
	flow := tp.LongestPipingFlowAt(lazy.expression, false)
	if flow == nil || !flow.MatchesExactly(isParameter, isBareOperator) {
		return node, nil, nil
	}

	forwarded := flow.Transformations().At(0)
	return ast.SkipParentheses(forwarded.Callee()), forwarded.TypeArguments(), parameterNode
}
