// Package checkerutils provides safe wrappers around checker operations.
package checkerutils

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// GetTypeAtLocation wraps checker.GetTypeAtLocation with node-kind and JSX safety guards.
// It returns nil when the node is nil, not an expression/type-node/declaration,
// a JSX tag name, or a JSX attribute name. It also recovers from checker panics
// (e.g. nil symbol dereferences on certain declaration nodes) and returns nil.
func GetTypeAtLocation(c *checker.Checker, node *ast.Node) (result *checker.Type) {
	if node == nil {
		return nil
	}

	// Guard against nodes with no parent (e.g. source file root)
	if node.Parent != nil {
		// Skip JSX tag names (JsxOpeningElement, JsxClosingElement, JsxSelfClosingElement)
		if ast.IsJsxTagName(node) {
			return nil
		}

		// Skip JSX attribute names
		if ast.IsJsxAttribute(node.Parent) && node.Parent.Name() == node {
			return nil
		}
	}

	if !ast.IsExpression(node) && !ast.IsTypeNode(node) && !ast.IsDeclaration(node) {
		return nil
	}

	// Skip sub-expressions inside interface heritage clauses (e.g. the identifiers and
	// property-access nodes within "extends Foo.Bar<T>"). Calling GetTypeAtLocation on
	// these nodes forces the checker to resolve them as value expressions, which can
	// trigger spurious errors (e.g. TS2689 "Cannot extend an interface") because the
	// referenced symbols may only exist in the type namespace.
	if isInsideInterfaceHeritageExpression(node) {
		return nil
	}

	defer func() {
		if r := recover(); r != nil {
			result = nil
		}
	}()

	return c.GetTypeAtLocation(node)
}

// isInsideInterfaceHeritageExpression reports whether node is an identifier or
// property-access that is a sub-expression of an ExpressionWithTypeArguments
// inside a non-class heritage clause (i.e. an interface "extends" clause).
// The ExpressionWithTypeArguments node itself is NOT skipped — only its
// inner expression children, which the checker would incorrectly resolve as
// value expressions.
func isInsideInterfaceHeritageExpression(node *ast.Node) bool {
	if node.Kind != ast.KindIdentifier && node.Kind != ast.KindPropertyAccessExpression {
		return false
	}
	// Walk up through identifiers and property-access expressions to find
	// the enclosing ExpressionWithTypeArguments.
	for n := node.Parent; n != nil; n = n.Parent {
		switch n.Kind {
		case ast.KindPropertyAccessExpression:
			// Keep walking up through nested property accesses.
			continue
		case ast.KindExpressionWithTypeArguments:
			// Found it — now check if it's inside a non-class heritage clause.
			// For classes the checker handles this correctly; the problem is
			// only with interfaces (and type-only heritage clauses).
			if n.Parent != nil && ast.IsHeritageClause(n.Parent) {
				grandparent := n.Parent.Parent
				if grandparent != nil && grandparent.Kind == ast.KindInterfaceDeclaration {
					return true
				}
			}
			return false
		default:
			return false
		}
	}
	return false
}
