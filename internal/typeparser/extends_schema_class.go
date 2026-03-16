package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// SchemaClassResult holds the parsed result of a class extending Schema.Class or Schema.RequestClass.
type SchemaClassResult struct {
	ClassName    *ast.Node // The class name identifier
	SelfTypeNode *ast.Node // The Self type argument node (first type arg of the inner call)
}

// ExtendsSchemaClass checks if a class declaration extends Schema.Class<Self>("name")({}).
// It detects the double-call pattern:
//
//	class X extends Schema.Class<X>("name")({}) {}
//
// where the ExpressionWithTypeArguments.expression is a CallExpression (outer call)
// whose own .expression is also a CallExpression (inner call) with type arguments,
// and the inner call resolves to Schema.Class.
//
// Returns nil if the class does not extend Schema.Class.
func ExtendsSchemaClass(c *checker.Checker, classNode *ast.Node) *SchemaClassResult {
	if c == nil || classNode == nil {
		return nil
	}
	links := GetEffectLinks(c)
	return Cached(&links.ExtendsSchemaClass, classNode, func() *SchemaClassResult {
		return extendsSchemaClassLike(c, classNode, "Class")
	})
}

// ExtendsSchemaRequestClass checks if a class declaration extends Schema.RequestClass<Self>("name")({}).
// Same double-call pattern as ExtendsSchemaClass but for Schema.RequestClass.
//
// Returns nil if the class does not extend Schema.RequestClass.
func ExtendsSchemaRequestClass(c *checker.Checker, classNode *ast.Node) *SchemaClassResult {
	if c == nil || classNode == nil {
		return nil
	}
	links := GetEffectLinks(c)
	return Cached(&links.ExtendsSchemaRequestClass, classNode, func() *SchemaClassResult {
		return extendsSchemaClassLike(c, classNode, "RequestClass")
	})
}

// extendsSchemaClassLike is the shared implementation for ExtendsSchemaClass and ExtendsSchemaRequestClass.
func extendsSchemaClassLike(c *checker.Checker, classNode *ast.Node, memberName string) *SchemaClassResult {
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

		// The inner call must have type arguments (Schema.Class<Self>())
		if innerCall.TypeArguments == nil || len(innerCall.TypeArguments.Nodes) == 0 {
			continue
		}

		// Check if the inner call's expression resolves to Schema.<memberName>
		if innerCall.Expression == nil {
			continue
		}
		if !IsNodeReferenceToEffectSchemaModuleApi(c, innerCall.Expression, memberName) {
			continue
		}

		return &SchemaClassResult{
			ClassName:    classNode.Name(),
			SelfTypeNode: innerCall.TypeArguments.Nodes[0],
		}
	}

	return nil
}
