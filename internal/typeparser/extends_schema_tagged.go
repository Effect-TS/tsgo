package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// SchemaTaggedResult holds the parsed result of a class extending Schema.TaggedClass/TaggedError/TaggedRequest.
type SchemaTaggedResult struct {
	ClassName        *ast.Node // The class name identifier
	SelfTypeNode     *ast.Node // The Self type argument node (first type arg of the inner call)
	KeyStringLiteral *ast.Node // The identifier arg from the inner call (first arg), or nil
	TagStringLiteral *ast.Node // The tag arg from the outer call (first arg), or nil
}

// extendsSchemaTagged checks if a class declaration extends Schema.<memberName>
// with the double-call pattern:
//
//	class X extends Schema.TaggedClass<X>("identifier")("tag", { ... }) {}
//
// where the ExpressionWithTypeArguments.expression is a CallExpression (outer call)
// whose own .expression is also a CallExpression (inner call) with type arguments,
// and the inner call resolves to Schema.<memberName>.
//
// Returns nil if the class does not match.
func extendsSchemaTagged(c *checker.Checker, classNode *ast.Node, memberName string) *SchemaTaggedResult {
	if c == nil || classNode == nil {
		return nil
	}

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

		// The inner call must have type arguments (Schema.TaggedClass<Self>())
		if innerCall.TypeArguments == nil || len(innerCall.TypeArguments.Nodes) == 0 {
			continue
		}

		// Check if the inner call's expression resolves to Schema.<memberName>
		if innerCall.Expression == nil {
			continue
		}
		if !(&TypeParser{program: c.Program(), checker: c}).IsNodeReferenceToEffectSchemaModuleApi(innerCall.Expression, memberName) {
			continue
		}

		// Extract keyStringLiteral from inner call's first argument (if it's a string literal)
		var keyStringLiteral *ast.Node
		if innerCall.Arguments != nil && len(innerCall.Arguments.Nodes) > 0 {
			arg := innerCall.Arguments.Nodes[0]
			if ast.IsStringLiteral(arg) {
				keyStringLiteral = arg
			}
		}

		// Extract tagStringLiteral from outer call's first argument (if it's a string literal)
		var tagStringLiteral *ast.Node
		if outerCall.Arguments != nil && len(outerCall.Arguments.Nodes) > 0 {
			arg := outerCall.Arguments.Nodes[0]
			if ast.IsStringLiteral(arg) {
				tagStringLiteral = arg
			}
		}

		return &SchemaTaggedResult{
			ClassName:        classNode.Name(),
			SelfTypeNode:     innerCall.TypeArguments.Nodes[0],
			KeyStringLiteral: keyStringLiteral,
			TagStringLiteral: tagStringLiteral,
		}
	}

	return nil
}

// ExtendsSchemaTaggedClass checks if a class declaration extends Schema.TaggedClass<T>("identifier")("tag", { ... }).
func (tp *TypeParser) ExtendsSchemaTaggedClass(classNode *ast.Node) *SchemaTaggedResult {
	if tp == nil || tp.checker == nil || classNode == nil {
		return nil
	}
	links := tp.GetEffectLinks()
	return Cached(&links.ExtendsSchemaTaggedClass, classNode, func() *SchemaTaggedResult {
		return extendsSchemaTagged(tp.checker, classNode, "TaggedClass")
	})
}

// ExtendsSchemaTaggedError checks if a class declaration extends Schema.TaggedError<T>("identifier")("tag", { ... }).
func (tp *TypeParser) ExtendsSchemaTaggedError(classNode *ast.Node) *SchemaTaggedResult {
	if tp == nil || tp.checker == nil || classNode == nil {
		return nil
	}
	links := tp.GetEffectLinks()
	return Cached(&links.ExtendsSchemaTaggedError, classNode, func() *SchemaTaggedResult {
		return extendsSchemaTagged(tp.checker, classNode, "TaggedError")
	})
}

// ExtendsSchemaTaggedRequest checks if a class declaration extends Schema.TaggedRequest<T>("identifier")("tag", { ... }).
func (tp *TypeParser) ExtendsSchemaTaggedRequest(classNode *ast.Node) *SchemaTaggedResult {
	if tp == nil || tp.checker == nil || classNode == nil {
		return nil
	}
	links := tp.GetEffectLinks()
	return Cached(&links.ExtendsSchemaTaggedRequest, classNode, func() *SchemaTaggedResult {
		return extendsSchemaTagged(tp.checker, classNode, "TaggedRequest")
	})
}
