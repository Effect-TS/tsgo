package typeparser

import "github.com/microsoft/TypeScript/tsc/shim/ast"

// ObjectLiteralPropertyFlags tells what ObjectLiteralProperty learned about a
// property besides its initializer. None means the node is an object literal
// that certainly has no property with that name.
type ObjectLiteralPropertyFlags uint8

const (
	ObjectLiteralPropertyNone ObjectLiteralPropertyFlags = 0
	// The node is not an object literal (under any outer expressions).
	ObjectLiteralPropertyNotObjectLiteral ObjectLiteralPropertyFlags = 1 << (iota - 1)
	// A member has the name.
	ObjectLiteralPropertyDeclared
	// The last member with the name is a shorthand, method or accessor.
	ObjectLiteralPropertyNotAssignment
	// A spread after the last member with the name, or anywhere when no member has it.
	ObjectLiteralPropertyMaybeSpread
	// A computed name that is not a literal after the last member with the name,
	// or anywhere when no member has it.
	ObjectLiteralPropertyMaybeComputed
)

// ObjectLiteralProperty looks up a property of an object literal (under any
// parentheses, type assertions or satisfies). It returns the initializer the
// property is known to hold: the last property assignment with that name,
// provided nothing after it can override it. The flags tell why the
// initializer is nil: the name is absent (None), the value is not a plain
// assignment, or a later spread or unresolved computed name may set it.
// The initializer is non-nil exactly when the flags are Declared alone.
func ObjectLiteralProperty(node *ast.Node, name string) (*ast.Node, ObjectLiteralPropertyFlags) {
	if node == nil {
		return nil, ObjectLiteralPropertyNotObjectLiteral
	}
	node = ast.SkipOuterExpressions(node, ast.OEKAll)
	if node.Kind != ast.KindObjectLiteralExpression {
		return nil, ObjectLiteralPropertyNotObjectLiteral
	}
	properties := node.AsObjectLiteralExpression().Properties
	if properties == nil {
		return nil, ObjectLiteralPropertyNone
	}
	var initializer *ast.Node
	flags := ObjectLiteralPropertyNone
	for _, property := range properties.Nodes {
		if property.Kind == ast.KindSpreadAssignment {
			initializer = nil
			flags |= ObjectLiteralPropertyMaybeSpread
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
			flags |= ObjectLiteralPropertyMaybeComputed
		case text != name:
		case property.Kind == ast.KindPropertyAssignment:
			initializer = property.AsPropertyAssignment().Initializer
			flags = ObjectLiteralPropertyDeclared
		default:
			initializer = nil
			flags = ObjectLiteralPropertyDeclared | ObjectLiteralPropertyNotAssignment
		}
	}
	return initializer, flags
}

// ObjectLiteralPropertyInitializer returns the initializer that a property of
// an object literal is known to hold, as ObjectLiteralProperty does, or nil
// when the value is absent or unknown.
func ObjectLiteralPropertyInitializer(node *ast.Node, name string) *ast.Node {
	initializer, _ := ObjectLiteralProperty(node, name)
	return initializer
}
