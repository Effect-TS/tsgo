package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
)

// SqlModelClassResult holds the parsed result of a class extending Model.Class.
type SqlModelClassResult struct {
	ClassName    *ast.Node // The class name identifier
	SelfTypeNode *ast.Node // The Self type argument node (first type arg of the inner call)
}

// ExtendsEffectSqlModelClass checks if a class declaration extends Model.Class<Self>(...)({...}).
// It detects the double-call pattern:
//
//	class X extends Model.Class<X>("name")({}) {}
//
// where the ExpressionWithTypeArguments.expression is a CallExpression (outer call)
// whose own .expression is also a CallExpression (inner call) with type arguments,
// and the inner call resolves to @effect/sql Model.Class.
func (tp *TypeParser) ExtendsEffectSqlModelClass(classNode *ast.Node) *SqlModelClassResult {
	if tp == nil || tp.checker == nil || classNode == nil {
		return nil
	}

	links := tp.GetEffectLinks()
	return Cached(&links.ExtendsEffectSqlModelClass, classNode, func() *SqlModelClassResult {
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

			outerCallNode := ewta.Expression
			if !ast.IsCallExpression(outerCallNode) {
				continue
			}
			outerCall := outerCallNode.AsCallExpression()
			if outerCall == nil {
				continue
			}

			innerCallNode := outerCall.Expression
			if innerCallNode == nil || !ast.IsCallExpression(innerCallNode) {
				continue
			}
			innerCall := innerCallNode.AsCallExpression()
			if innerCall == nil {
				continue
			}

			if innerCall.TypeArguments == nil || len(innerCall.TypeArguments.Nodes) == 0 {
				continue
			}

			if innerCall.Expression == nil {
				continue
			}
			if !tp.IsNodeReferenceToEffectSqlModelModuleApi(innerCall.Expression, "Class") {
				continue
			}

			return &SqlModelClassResult{
				ClassName:    classNode.Name(),
				SelfTypeNode: innerCall.TypeArguments.Nodes[0],
			}
		}

		return nil
	})
}
