package fixables

import (
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// replacementForExpression returns the node that replaces position with value.
// When position is the concise body of an arrow function and value begins with
// an object literal (`{ a }`, `{ a } as const`, `{ a } satisfies T`), the body
// would parse as a block, so value is parenthesized.
func replacementForExpression(tracker *rewriter.Tracker, position *ast.Node, value *ast.Node) *ast.Node {
	if !isConciseArrowBody(position) || leadingTokenKind(value) != ast.KindObjectLiteralExpression {
		return value
	}
	return tracker.NewParenthesizedExpression(tracker.DeepCloneNode(value))
}

// isConciseArrowBody reports whether node is the expression body of an arrow
// function.
func isConciseArrowBody(node *ast.Node) bool {
	parent := node.Parent
	return parent != nil && parent.Kind == ast.KindArrowFunction && parent.Body() == node
}
