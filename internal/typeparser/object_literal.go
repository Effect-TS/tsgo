package typeparser

import "github.com/microsoft/TypeScript/tsc/shim/ast"

// ObjectLiteralPropertyInitializer returns the initializer of an explicitly
// assigned identifier or string-literal property in an object literal.
func ObjectLiteralPropertyInitializer(node *ast.Node, name string) *ast.Node {
	node = ast.SkipParentheses(node)
	if node == nil || node.Kind != ast.KindObjectLiteralExpression {
		return nil
	}
	object := node.AsObjectLiteralExpression()
	if object == nil || object.Properties == nil {
		return nil
	}
	for _, propertyNode := range object.Properties.Nodes {
		if propertyNode == nil || propertyNode.Kind != ast.KindPropertyAssignment {
			continue
		}
		property := propertyNode.AsPropertyAssignment()
		if property == nil || property.Name() == nil {
			continue
		}
		propertyName := property.Name()
		switch propertyName.Kind {
		case ast.KindIdentifier:
			if propertyName.AsIdentifier().Text == name {
				return property.Initializer
			}
		case ast.KindStringLiteral:
			if propertyName.AsStringLiteral().Text == name {
				return property.Initializer
			}
		}
	}
	return nil
}
