package typeparser

import (
	"slices"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// UsesFunctionOwnBindings reports whether root, a node inside the function fn,
// reads a binding that fn itself provides:
//   - this or super whose this-container (ast.GetThisContainer) is fn;
//   - new.target whose container (ast.GetNewTargetContainer) is fn;
//   - fn's implicit arguments object, shorthand { arguments } included;
//   - fn's own name, when fn is a named function expression.
//
// fn's parameters are not included: callers check them with ReferencesToSymbol.
// An arrow function binds none of these, so it always returns false.
func (tp *TypeParser) UsesFunctionOwnBindings(fn, root *ast.Node) bool {
	if tp == nil || tp.checker == nil || fn == nil || root == nil || fn.Kind == ast.KindArrowFunction {
		return false
	}
	if fn.Kind == ast.KindFunctionExpression {
		if name := fn.Name(); name != nil && tp.ReferencesToSymbol(root, tp.GetSymbolAtLocation(name)).Len() > 0 {
			return true
		}
	}
	for _, reference := range tp.ReferencesToSymbol(root, tp.checker.GetArgumentsSymbol()).All() {
		if ast.GetThisContainer(reference, false, false) == fn {
			return true
		}
	}
	return usesThisContainerKeywords(fn, root)
}

// usesThisContainerKeywords reports whether root reads this, super or new.target
// of fn. It skips the parts of nested non-arrow this-containers that belong to
// them; ast.GetThisContainer still decides each keyword it finds.
func usesThisContainerKeywords(fn, root *ast.Node) bool {
	var visit ast.Visitor
	visitModifiers := func(modifiers *ast.ModifierList) bool {
		return modifiers != nil && slices.ContainsFunc(modifiers.Nodes, visit)
	}
	visit = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		switch node.Kind {
		case ast.KindThisKeyword, ast.KindSuperKeyword:
			return ast.GetThisContainer(node, false, false) == fn
		case ast.KindMetaProperty:
			return node.AsMetaProperty().KeywordToken == ast.KindNewKeyword && ast.GetNewTargetContainer(node) == fn
		case ast.KindFunctionExpression, ast.KindFunctionDeclaration:
			// Nothing of a nested function is evaluated in the enclosing scope.
			if node != fn {
				return false
			}
		case ast.KindMethodDeclaration, ast.KindGetAccessor, ast.KindSetAccessor, ast.KindConstructor,
			ast.KindPropertyDeclaration, ast.KindClassStaticBlockDeclaration:
			// A member's decorators, its parameters' decorators and its computed
			// name are evaluated in the enclosing scope; the rest belongs to it.
			if node != fn {
				if visitModifiers(node.Modifiers()) {
					return true
				}
				if name := node.Name(); name != nil && name.Kind == ast.KindComputedPropertyName && visit(name) {
					return true
				}
				if ast.IsFunctionLike(node) {
					for _, parameter := range node.Parameters() {
						if visitModifiers(parameter.Modifiers()) {
							return true
						}
					}
				}
				return false
			}
		}
		//ets:ast-walk scans for this, super and new.target keywords, which have no symbol to look up
		return node.ForEachChild(visit)
	}
	return visit(root)
}
