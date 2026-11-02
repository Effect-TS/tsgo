package typeparser

import "github.com/microsoft/TypeScript/tsc/shim/ast"

// ObjectLiteralPropertyInitializer returns the initializer that a property of
// an object literal (under any parentheses, type assertions or satisfies) is
// known to hold: the last property assignment with that name, provided nothing
// after it can override it. A later spread, a later shorthand, method or
// accessor with the same name, or a later computed name that is not a literal
// makes the value unknown, and the result nil.
func ObjectLiteralPropertyInitializer(node *ast.Node, name string) *ast.Node {
	if node == nil {
		return nil
	}
	node = ast.SkipOuterExpressions(node, ast.OEKAll)
	if node.Kind != ast.KindObjectLiteralExpression {
		return nil
	}
	properties := node.AsObjectLiteralExpression().Properties
	if properties == nil {
		return nil
	}
	var initializer *ast.Node
	for _, property := range properties.Nodes {
		if property.Kind == ast.KindSpreadAssignment {
			initializer = nil
			continue
		}
		propertyName := property.Name()
		if propertyName == nil {
			continue
		}
		text, ok := ast.TryGetTextOfPropertyName(propertyName)
		switch {
		case !ok:
			initializer = nil
		case text != name:
		case property.Kind == ast.KindPropertyAssignment:
			initializer = property.AsPropertyAssignment().Initializer
		default:
			initializer = nil
		}
	}
	return initializer
}
