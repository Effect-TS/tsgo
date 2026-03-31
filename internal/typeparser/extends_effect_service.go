package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
)

// EffectServiceResult holds the parsed result of a class extending Effect.Service.
type EffectServiceResult struct {
	ClassName        *ast.Node // The class name identifier
	SelfTypeNode     *ast.Node // The Self type argument node (first type arg of the inner call)
	Options          *ast.Node // The options expression (second argument of the outer call), or nil
	KeyStringLiteral *ast.Node // The key string literal from the outer call's first argument, or nil
}

// ExtendsEffectService checks if a class declaration extends Effect.Service<Self>()(key, options).
// It detects the double-call pattern:
//
//	class X extends Effect.Service<X>()("key", { ... }) {}
//
// where the ExpressionWithTypeArguments.expression is a CallExpression (outer call)
// whose own .expression is also a CallExpression (inner call) with type arguments,
// and the inner call resolves to Effect.Service.
//
// Returns nil if the class does not extend Effect.Service.
func (tp *TypeParser) ExtendsEffectService(classNode *ast.Node) *EffectServiceResult {
	if tp == nil || tp.checker == nil || classNode == nil {
		return nil
	}

	return Cached(&tp.links.ExtendsEffectService, classNode, func() *EffectServiceResult {
		// Must have a name
		if classNode.Name() == nil {
			return nil
		}

		heritageElements := ast.GetExtendsHeritageClauseElements(classNode)
		if len(heritageElements) == 0 {
			return nil
		}

		for _, element := range heritageElements {
			if element == nil {
				continue
			}

			ewta := element.AsExpressionWithTypeArguments()
			if ewta == nil || ewta.Expression == nil {
				continue
			}

			// The expression should be a CallExpression (the outer call)
			outerCallNode := ewta.Expression
			if !ast.IsCallExpression(outerCallNode) {
				continue
			}
			outerCall := outerCallNode.AsCallExpression()
			if outerCall == nil {
				continue
			}

			// The outer call's expression should also be a CallExpression (the inner call)
			innerCallNode := outerCall.Expression
			if innerCallNode == nil || !ast.IsCallExpression(innerCallNode) {
				continue
			}
			innerCall := innerCallNode.AsCallExpression()
			if innerCall == nil {
				continue
			}

			// The inner call must have type arguments (Effect.Service<Self>())
			if innerCall.TypeArguments == nil || len(innerCall.TypeArguments.Nodes) == 0 {
				continue
			}

			// Check if the inner call's expression resolves to Effect.Service
			if innerCall.Expression == nil {
				continue
			}
			if !tp.IsNodeReferenceToEffectModuleApi(innerCall.Expression, "Service") {
				continue
			}

			// Extract the key string literal from outer call's first argument
			var keyStringLiteral *ast.Node
			if outerCall.Arguments != nil && len(outerCall.Arguments.Nodes) > 0 {
				arg := outerCall.Arguments.Nodes[0]
				if ast.IsStringLiteral(arg) {
					keyStringLiteral = arg
				}
			}

			// Extract the options expression (second argument of the outer call, if present)
			var options *ast.Node
			if outerCall.Arguments != nil && len(outerCall.Arguments.Nodes) >= 2 {
				options = outerCall.Arguments.Nodes[1]
			}

			return &EffectServiceResult{
				ClassName:        classNode.Name(),
				SelfTypeNode:     innerCall.TypeArguments.Nodes[0],
				Options:          options,
				KeyStringLiteral: keyStringLiteral,
			}
		}

		return nil
	})
}
