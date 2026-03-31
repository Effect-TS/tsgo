package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
)

// DataTaggedErrorResult holds the parsed result of a class extending Data.TaggedError.
type DataTaggedErrorResult struct {
	ClassName        *ast.Node // The class name identifier
	KeyStringLiteral *ast.Node // The key string literal from the call's first argument, or nil
}

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

	links := tp.GetEffectLinks()
	return Cached(&links.ExtendsDataTaggedError, classNode, func() *DataTaggedErrorResult {
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

			return &DataTaggedErrorResult{
				ClassName:        classNode.Name(),
				KeyStringLiteral: keyStringLiteral,
			}
		}

		return nil
	})
}
