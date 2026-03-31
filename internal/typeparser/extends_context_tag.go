package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
)

// ContextTagResult holds the parsed result of a class extending Context.Tag.
type ContextTagResult struct {
	ClassName        *ast.Node // The class name identifier
	SelfTypeNode     *ast.Node // The Self type argument node (first type arg of the outer call)
	KeyStringLiteral *ast.Node // The key string literal from the inner call's first argument, or nil
}

// ExtendsContextTag checks if a class declaration extends Context.Tag("key")<Self, Shape>().
// It detects the pattern:
//
//	class X extends Context.Tag("key")<X, Shape>() {}
//
// where the ExpressionWithTypeArguments.expression is a CallExpression (outer call)
// that has type arguments <Self, Shape>, and whose own .expression is a CallExpression
// (inner call: Context.Tag("key")), and the inner call's expression resolves to Context.Tag.
//
// Returns nil if the class does not extend Context.Tag.
func (tp *TypeParser) ExtendsContextTag(classNode *ast.Node) *ContextTagResult {
	if tp == nil || tp.checker == nil || classNode == nil {
		return nil
	}

	links := tp.links
	return Cached(&links.ExtendsContextTag, classNode, func() *ContextTagResult {
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

			// The expression should be a CallExpression (the outer call with type arguments)
			outerCallNode := ewta.Expression
			if !ast.IsCallExpression(outerCallNode) {
				continue
			}
			outerCall := outerCallNode.AsCallExpression()
			if outerCall == nil {
				continue
			}

			// The outer call must have type arguments (<Self, Shape>)
			if outerCall.TypeArguments == nil || len(outerCall.TypeArguments.Nodes) == 0 {
				continue
			}

			// The outer call's expression should also be a CallExpression (the inner call: Context.Tag("key"))
			innerCallNode := outerCall.Expression
			if innerCallNode == nil || !ast.IsCallExpression(innerCallNode) {
				continue
			}
			innerCall := innerCallNode.AsCallExpression()
			if innerCall == nil {
				continue
			}

			// Check if the inner call's expression resolves to Context.Tag
			if innerCall.Expression == nil {
				continue
			}
			if !tp.IsNodeReferenceToEffectContextModuleApi(innerCall.Expression, "Tag") {
				continue
			}

			// Extract key string literal from inner call's first argument
			var keyStringLiteral *ast.Node
			if innerCall.Arguments != nil && len(innerCall.Arguments.Nodes) > 0 {
				arg := innerCall.Arguments.Nodes[0]
				if ast.IsStringLiteral(arg) {
					keyStringLiteral = arg
				}
			}

			return &ContextTagResult{
				ClassName:        classNode.Name(),
				SelfTypeNode:     outerCall.TypeArguments.Nodes[0],
				KeyStringLiteral: keyStringLiteral,
			}
		}

		return nil
	})
}
