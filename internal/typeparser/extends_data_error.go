package typeparser

import "github.com/microsoft/TypeScript/tsc/shim/ast"

type DataErrorResult struct {
	className      *ast.Node
	fieldsTypeNode *ast.Node
}

func (r DataErrorResult) ClassName() *ast.Node { return r.className }

func (r DataErrorResult) FieldsTypeNode() *ast.Node { return r.fieldsTypeNode }

func (tp *TypeParser) ExtendsDataError(classNode *ast.Node) *DataErrorResult {
	if tp == nil || tp.checker == nil || classNode == nil {
		return nil
	}
	links := tp.links
	return Cached(&links.ExtendsDataError, classNode, func() *DataErrorResult {
		for _, element := range ast.GetExtendsHeritageClauseElements(classNode) {
			if element == nil {
				continue
			}
			ewta := element.AsExpressionWithTypeArguments()
			if ewta == nil || ewta.Expression == nil || !tp.IsNodeReferenceToEffectDataModuleApi(ewta.Expression, "Error") {
				continue
			}
			// Heritage namespace lookup for entity-name expressions can ignore a shadowing value binding.
			if ast.IsEntityNameExpression(ewta.Expression) {
				root := ast.GetFirstIdentifier(ewta.Expression)
				if ast.IsIdentifier(root) {
					valueSymbol := tp.referenceSymbolFrom(root, tp.checker.GetResolvedSymbol(root))
					if !sameSymbolReference(tp.checker, valueSymbol, tp.ReferenceSymbolAtNode(root)) {
						continue
					}
				}
			}
			var fieldsTypeNode *ast.Node
			if ewta.TypeArguments != nil && len(ewta.TypeArguments.Nodes) > 0 {
				fieldsTypeNode = ewta.TypeArguments.Nodes[0]
			}
			return &DataErrorResult{
				className:      classNode.Name(),
				fieldsTypeNode: fieldsTypeNode,
			}
		}
		return nil
	})
}
