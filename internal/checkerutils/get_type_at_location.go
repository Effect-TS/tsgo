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

	defer func() {
		if r := recover(); r != nil {
			result = nil
		}
	}()

	return c.GetTypeAtLocation(node)
}
