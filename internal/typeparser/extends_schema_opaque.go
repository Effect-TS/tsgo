package typeparser

import "github.com/microsoft/typescript-go/shim/ast"

// SchemaOpaqueResult holds the type argument from a class extending Schema.Opaque.
type SchemaOpaqueResult struct {
	SelfTypeNode *ast.Node
}

// ExtendsSchemaOpaque checks for the exact Schema.Opaque double-call heritage shape:
//
//	class X extends Schema.Opaque<X>()(schema) {}
func (tp *TypeParser) ExtendsSchemaOpaque(classNode *ast.Node) *SchemaOpaqueResult {
	if tp == nil || tp.checker == nil || classNode == nil {
		return nil
	}
	links := tp.links
	return Cached(&links.ExtendsSchemaOpaque, classNode, func() *SchemaOpaqueResult {
		for _, element := range ast.GetExtendsHeritageClauseElements(classNode) {
			if element == nil {
				continue
			}
			extendsExpression := element.AsExpressionWithTypeArguments()
			if extendsExpression == nil || !ast.IsCallExpression(extendsExpression.Expression) {
				continue
			}
			outerCall := extendsExpression.Expression.AsCallExpression()
			if outerCall == nil || !ast.IsCallExpression(outerCall.Expression) {
				continue
			}
			innerCall := outerCall.Expression.AsCallExpression()
			if innerCall == nil || innerCall.Expression == nil || innerCall.TypeArguments == nil || len(innerCall.TypeArguments.Nodes) != 1 {
				continue
			}
			if tp.IsNodeReferenceToEffectSchemaModuleApi(innerCall.Expression, "Opaque") {
				return &SchemaOpaqueResult{SelfTypeNode: innerCall.TypeArguments.Nodes[0]}
			}
		}
		return nil
	})
}
