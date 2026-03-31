package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
)

// ServiceMapServiceResult holds the parsed result of a class extending ServiceMap.Service.
type ServiceMapServiceResult struct {
	ClassName        *ast.Node // The class name identifier
	SelfTypeNode     *ast.Node // The Self type argument node (first type arg of the inner call)
	KeyStringLiteral *ast.Node // The key string literal from the outer call's first argument, or nil
}

// ExtendsServiceMapService checks if a class declaration extends ServiceMap.Service<Self, Shape>()(key).
// It detects the double-call pattern:
//
//	class X extends ServiceMap.Service<X, Shape>()("key") {}
//
// where the ExpressionWithTypeArguments.expression is a CallExpression (outer call)
// whose own .expression is also a CallExpression (inner call) with type arguments,
// and the inner call resolves to ServiceMap.Service.
//
// Returns nil if the class does not extend ServiceMap.Service.
func (tp *TypeParser) ExtendsServiceMapService(classNode *ast.Node) *ServiceMapServiceResult {
	if tp == nil || tp.checker == nil || classNode == nil {
		return nil
	}

	links := tp.links
	return Cached(&links.ExtendsServiceMapService, classNode, func() *ServiceMapServiceResult {
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

			// The inner call must have type arguments (ServiceMap.Service<Self, Shape>())
			if innerCall.TypeArguments == nil || len(innerCall.TypeArguments.Nodes) == 0 {
				continue
			}

			// Check if the inner call's expression resolves to ServiceMap.Service
			if innerCall.Expression == nil {
				continue
			}
			if !tp.IsNodeReferenceToServiceMapModuleApi(innerCall.Expression, "Service") {
				continue
			}

			// Extract key string literal from outer call's first argument
			var keyStringLiteral *ast.Node
			if outerCall.Arguments != nil && len(outerCall.Arguments.Nodes) > 0 {
				arg := outerCall.Arguments.Nodes[0]
				if ast.IsStringLiteral(arg) {
					keyStringLiteral = arg
				}
			}

			return &ServiceMapServiceResult{
				ClassName:        classNode.Name(),
				SelfTypeNode:     innerCall.TypeArguments.Nodes[0],
				KeyStringLiteral: keyStringLiteral,
			}
		}

		return nil
	})
}
