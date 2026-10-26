package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// DataTaggedResult holds the parsed result of a class extending Data.TaggedError or Data.TaggedClass.
type DataTaggedResult struct {
	className        *ast.Node // The class name identifier, or nil for an anonymous declaration
	callExpression   *ast.Node // The matched Data.<member> call, always non-nil
	keyStringLiteral *ast.Node // The key string literal from the call's first argument, or nil
	fieldsTypeNode   *ast.Node
}

func (r DataTaggedResult) ClassName() *ast.Node { return r.className }

func (r DataTaggedResult) CallExpression() *ast.Node { return r.callExpression }

func (r DataTaggedResult) KeyStringLiteral() *ast.Node { return r.keyStringLiteral }

func (r DataTaggedResult) FieldsTypeNode() *ast.Node { return r.fieldsTypeNode }

// extendsDataTagged checks if a class declaration extends Data.<memberName>("key")<Fields>.
// It detects the pattern:
//
//	class X extends Data.TaggedError("key")<{ msg: string }> {}
//
// where the ExpressionWithTypeArguments.expression is a CallExpression (Data.<memberName>("key")),
// the call's expression is a PropertyAccessExpression resolving to Data.<memberName>,
// and the type arguments <Fields> are on the ExpressionWithTypeArguments.
//
// Returns nil if the class does not match.
func (tp *TypeParser) extendsDataTagged(classNode *ast.Node, memberName string) *DataTaggedResult {
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

		// The expression should be a CallExpression: Data.<memberName>("key")
		callNode := ewta.Expression
		if !ast.IsCallExpression(callNode) {
			continue
		}
		call := callNode.AsCallExpression()
		if call == nil {
			continue
		}

		// The call's expression should be a PropertyAccessExpression: Data.<memberName>
		if call.Expression == nil {
			continue
		}
		if !tp.IsNodeReferenceToEffectDataModuleApi(call.Expression, memberName) {
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

		return &DataTaggedResult{
			className:        classNode.Name(),
			callExpression:   callNode,
			keyStringLiteral: keyStringLiteral,
			fieldsTypeNode:   fieldsTypeNode,
		}
	}

	return nil
}

// ExtendsDataTaggedError checks if a class declaration extends Data.TaggedError("key")<Fields>.
func (tp *TypeParser) ExtendsDataTaggedError(classNode *ast.Node) *DataTaggedResult {
	if tp == nil || tp.checker == nil || classNode == nil {
		return nil
	}
	links := tp.links
	return Cached(&links.ExtendsDataTaggedError, classNode, func() *DataTaggedResult {
		return tp.extendsDataTagged(classNode, "TaggedError")
	})
}

// ExtendsDataTaggedClass checks if a class declaration extends Data.TaggedClass("tag")<Fields>.
func (tp *TypeParser) ExtendsDataTaggedClass(classNode *ast.Node) *DataTaggedResult {
	if tp == nil || tp.checker == nil || classNode == nil {
		return nil
	}
	links := tp.links
	return Cached(&links.ExtendsDataTaggedClass, classNode, func() *DataTaggedResult {
		return tp.extendsDataTagged(classNode, "TaggedClass")
	})
}
