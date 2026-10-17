package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func (tp *TypeParser) GetContextualType(node *ast.Node, flags checker.ContextFlags) *checker.Type {
	if shouldSkipInferredGeneratorContextualType(node, flags) {
		return nil
	}
	return tp.checker.GetContextualType(node, flags)
}

func shouldSkipInferredGeneratorContextualType(node *ast.Node, flags checker.ContextFlags) bool {
	if flags&checker.ContextFlagsIgnoreNodeInferences == 0 {
		return false
	}
	for current := node; current != nil; current = current.Parent {
		if current.Kind == ast.KindFunctionExpression && current.AsFunctionExpression().AsteriskToken != nil && current.Type() == nil {
			return true
		}
	}
	return false
}
