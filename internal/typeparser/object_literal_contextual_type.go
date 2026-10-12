package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func (tp *TypeParser) CachedObjectLiteralContextualType(node *ast.Node) *checker.Type {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindObjectLiteralExpression {
		return nil
	}
	return Cached(&tp.links.ObjectLiteralContextualType, node, func() *checker.Type {
		return tp.GetContextualType(node, checker.ContextFlagsNone)
	})
}

func (tp *TypeParser) CachedObjectLiteralConstraintContextualType(node *ast.Node) *checker.Type {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindObjectLiteralExpression {
		return nil
	}
	return Cached(&tp.links.ObjectLiteralConstraintContextualType, node, func() *checker.Type {
		return tp.GetContextualType(node, checker.ContextFlagsIgnoreNodeInferences)
	})
}
