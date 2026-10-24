package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// DataTaggedErrorResult holds the parsed result of a class extending Data.TaggedError.
type DataTaggedErrorResult struct {
	className        *ast.Node // The class name identifier, or nil for an anonymous declaration
	callExpression   *ast.Node // The matched Data.TaggedError call, always non-nil
	keyStringLiteral *ast.Node // The key string literal from the call's first argument, or nil
	fieldsTypeNode   *ast.Node
}

func (r DataTaggedErrorResult) ClassName() *ast.Node { return r.className }

func (r DataTaggedErrorResult) CallExpression() *ast.Node { return r.callExpression }

func (r DataTaggedErrorResult) KeyStringLiteral() *ast.Node { return r.keyStringLiteral }

func (r DataTaggedErrorResult) FieldsTypeNode() *ast.Node { return r.fieldsTypeNode }

// ExtendsDataTaggedError checks if a class declaration extends Data.TaggedError("key")<Fields>.
// It detects the pattern:
//
//	class X extends Data.TaggedError("key")<{ msg: string }> {}
//
// where the ExpressionWithTypeArguments.expression is a CallExpression (Data.TaggedError("key")),
// the call's expression is a PropertyAccessExpression resolving to Data.TaggedError,
// and the type arguments <Fields> are on the ExpressionWithTypeArguments.
//
// Returns nil if the class does not extend Data.TaggedError.
func (tp *TypeParser) ExtendsDataTaggedError(classNode *ast.Node) *DataTaggedErrorResult {
	if tp == nil || tp.checker == nil || classNode == nil {
		return nil
	}

	links := tp.links
	return Cached(&links.ExtendsDataTaggedError, classNode, func() *DataTaggedErrorResult {
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

			// The expression should be a CallExpression: Data.TaggedError("key")
			callNode := ewta.Expression
			if !ast.IsCallExpression(callNode) {
				continue
			}
			call := callNode.AsCallExpression()
			if call == nil {
				continue
			}

			// The call's expression should be a PropertyAccessExpression: Data.TaggedError
			if call.Expression == nil {
				continue
			}
			if !tp.IsNodeReferenceToEffectDataModuleApi(call.Expression, "TaggedError") {
				continue
			}

			// Extract key string literal from call's first argument
			var keyStringLiteral *ast.Node
			if call.Arguments != nil && len(call.Arguments.Nodes) > 0 {
				arg := call.Arguments.Nodes[0]
				if ast.IsStringLiteral(arg) {
					keyStringLiteral = arg
				}
			}

			var fieldsTypeNode *ast.Node
			if ewta.TypeArguments != nil && len(ewta.TypeArguments.Nodes) > 0 {
				fieldsTypeNode = ewta.TypeArguments.Nodes[0]
			}

			return &DataTaggedErrorResult{
				className:        classNode.Name(),
				callExpression:   callNode,
				keyStringLiteral: keyStringLiteral,
				fieldsTypeNode:   fieldsTypeNode,
			}
		}

		return nil
	})
}
