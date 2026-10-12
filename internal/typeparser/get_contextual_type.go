package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func (tp *TypeParser) GetContextualType(node *ast.Node, flags checker.ContextFlags) *checker.Type {
	return tp.checker.GetContextualType(node, flags)
}
