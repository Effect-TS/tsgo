// Package checkerutils provides safe wrappers around checker operations.
package checkerutils

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// GetTypeAtLocation wraps checker.GetTypeAtLocation with JSX safety guards.
// It returns nil when the node is nil, a JSX tag name, or a JSX attribute name.
func GetTypeAtLocation(c *checker.Checker, node *ast.Node) *checker.Type {
	if node == nil {
		return nil
	}

	if !ast.IsExpression(node) && !ast.IsTypeNode(node) {
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

	return c.GetTypeAtLocation(node)
}
