package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// TypeEquivalentNode returns the node whose type the checker computes when
// asked for the type of node, and true when that is a different node. Rules
// that type-check every node can skip node because the walk also visits the
// returned one:
//   - the name of a property access stands for the access (`b` in `a.b`);
//   - the name of a variable, parameter, property or object literal member
//     stands for its declaration;
//   - parentheses and `satisfies` stand for the wrapped expression.
func TypeEquivalentNode(node *ast.Node) (*ast.Node, bool) {
	parent := node.Parent
	if parent == nil {
		return node, false
	}
	switch node.Kind {
	case ast.KindIdentifier:
		if parent.Name() != node {
			return node, false
		}
		switch parent.Kind {
		case ast.KindPropertyAccessExpression:
			if !ast.IsPartOfTypeNode(parent) {
				return parent, true
			}
		case ast.KindVariableDeclaration, ast.KindParameter, ast.KindPropertyDeclaration,
			ast.KindPropertyAssignment, ast.KindShorthandPropertyAssignment:
			return parent, true
		}
	case ast.KindParenthesizedExpression, ast.KindSatisfiesExpression:
		return node.Expression(), true
	}
	return node, false
}
