package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// GetTypeAtLocation wraps checker.GetTypeAtLocation with node-kind and JSX safety guards.
// It returns nil when the node is nil, not an expression/type-node/declaration,
// a JSX tag name, or a JSX attribute name. It also recovers from checker panics
// (e.g. nil symbol dereferences on certain declaration nodes) and returns nil.
func GetTypeAtLocation(c *checker.Checker, node *ast.Node) (result *checker.Type) {
	if c == nil || node == nil {
		return nil
	}

	links := getEffectLinksRoot(c)
	return Cached(&links.TypeAtLocation, node, func() *checker.Type {
		return getTypeAtLocationUncached(c, node)
	})
}

func getTypeAtLocationUncached(c *checker.Checker, node *ast.Node) (result *checker.Type) {
	if node == nil {
		return nil
	}

	if node.Parent != nil {
		if ast.IsJsxTagName(node) {
			return nil
		}

		if ast.IsJsxAttribute(node.Parent) && node.Parent.Name() == node {
			return nil
		}
	}

	if !ast.IsExpression(node) && !ast.IsTypeNode(node) && !ast.IsDeclaration(node) {
		return nil
	}

	if isInsideTypeOnlyHeritageExpression(node) {
		return nil
	}

	defer func() {
		if r := recover(); r != nil {
			result = nil
		}
	}()

	return c.GetTypeAtLocation(node)
}

// isInsideTypeOnlyHeritageExpression reports whether node is an
// ExpressionWithTypeArguments or one of its identifier/property-access
// sub-expressions inside a type-only heritage clause. The checker can
// mis-resolve these as value expressions and emit bogus diagnostics.
func isInsideTypeOnlyHeritageExpression(node *ast.Node) bool {
	if node.Kind == ast.KindExpressionWithTypeArguments {
		return isTypeOnlyHeritageClause(node.Parent)
	}

	if node.Kind != ast.KindIdentifier && node.Kind != ast.KindPropertyAccessExpression {
		return false
	}

	for n := node.Parent; n != nil; n = n.Parent {
		switch n.Kind {
		case ast.KindPropertyAccessExpression:
			continue
		case ast.KindExpressionWithTypeArguments:
			return isTypeOnlyHeritageClause(n.Parent)
		default:
			return false
		}
	}

	return false
}

func isTypeOnlyHeritageClause(node *ast.Node) bool {
	if node == nil || !ast.IsHeritageClause(node) {
		return false
	}

	heritageClause := node.AsHeritageClause()
	container := node.Parent
	if container == nil {
		return false
	}

	if container.Kind == ast.KindInterfaceDeclaration {
		return true
	}

	return ast.IsClassLike(container) && heritageClause.Token == ast.KindImplementsKeyword
}
