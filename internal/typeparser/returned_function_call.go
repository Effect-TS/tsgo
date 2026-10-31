package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// ReturnedFunctionCallOwner returns the name of the variable declaration whose
// declared type returns the function that call invokes. For run(work) after
// const run = Effect.runPromiseWith(context), that is the name of Effect's own
// runPromiseWith declaration, not the local run binding. It follows the
// resolved signature's declaration out of the enclosing function types, so it
// returns nil when the called function is not declared as another function's
// return type.
func (tp *TypeParser) ReturnedFunctionCallOwner(call *ast.Node) *ast.Node {
	if tp == nil || tp.checker == nil || call == nil || call.Kind != ast.KindCallExpression {
		return nil
	}
	resolved := tp.checker.GetResolvedSignature(call)
	if resolved == nil {
		return nil
	}
	declaration := rawSignature(resolved).Declaration()
	if declaration == nil || declaration.Parent == nil {
		return nil
	}
	parent := declaration.Parent
	if (parent.Kind != ast.KindFunctionType && parent.Kind != ast.KindCallSignature) || parent.Type() != declaration {
		return nil
	}
	for current := parent; current != nil; current = current.Parent {
		switch current.Kind {
		case ast.KindFunctionType, ast.KindCallSignature, ast.KindTypeLiteral, ast.KindParenthesizedType:
			continue
		case ast.KindVariableDeclaration:
			return current.Name()
		}
		return nil
	}
	return nil
}
