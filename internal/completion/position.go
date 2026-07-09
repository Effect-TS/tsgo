package completion

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// ExtendsClassCompletionData holds the parsed context for a completion
// triggered in the extends clause of a class declaration.
type ExtendsClassCompletionData struct {
	AccessedObject    *ast.Node
	ClassDeclaration  *ast.Node
	ClassName         *ast.Node
	ReplacementStart  int
	ReplacementLength int
}

// AccessedExpressionResult is the result from parsing an accessed expression at a cursor position.
type AccessedExpressionResult struct {
	AccessedObject    *ast.Node
	OuterNode         *ast.Node
	ReplacementStart  int
	ReplacementLength int
}

// ParseAccessedExpressionForCompletion finds the accessed expression at the given cursor position.
// It handles three cases:
//   - extends Schema.Tag| — cursor after a property name in a PropertyAccessExpression
//   - extends Schema.| — cursor right after the dot
//   - extends Schema| — cursor after a standalone identifier
func ParseAccessedExpressionForCompletion(sf *ast.SourceFile, position int) *AccessedExpressionResult {
	precedingToken := astnav.FindPrecedingToken(sf, position)
	if precedingToken == nil {
		return nil
	}

	var accessedObject *ast.Node
	var outerNode *ast.Node
	var replacementStart int
	replacementLength := 0

	switch {
	case ast.IsIdentifier(precedingToken) && precedingToken.Parent != nil &&
		ast.IsPropertyAccessExpression(precedingToken.Parent):
		// extends Schema.Tag|
		spanStart := astnav.GetStartOfNode(precedingToken.Parent, sf, false)
		replacementStart = spanStart
		replacementLength = precedingToken.End() - spanStart
		prop := precedingToken.Parent.AsPropertyAccessExpression()
		accessedObject = prop.Expression
		outerNode = precedingToken.Parent
	case ast.IsTokenKind(precedingToken.Kind) && precedingToken.Kind == ast.KindDotToken &&
		precedingToken.Parent != nil && ast.IsPropertyAccessExpression(precedingToken.Parent):
		// extends Schema.|
		spanStart := astnav.GetStartOfNode(precedingToken.Parent, sf, false)
		replacementStart = spanStart
		replacementLength = precedingToken.End() - spanStart
		prop := precedingToken.Parent.AsPropertyAccessExpression()
		accessedObject = prop.Expression
		outerNode = precedingToken.Parent
	case ast.IsIdentifier(precedingToken) && precedingToken.Parent != nil:
		// extends Schema|
		spanStart := astnav.GetStartOfNode(precedingToken, sf, false)
		replacementStart = spanStart
		replacementLength = precedingToken.End() - spanStart
		accessedObject = precedingToken
		outerNode = precedingToken
	default:
		return nil
	}

	// Skip if accessed object is inside an import declaration
	importDecl := ast.FindAncestor(accessedObject, ast.IsImportDeclaration)
	if importDecl != nil {
		return nil
	}

	return &AccessedExpressionResult{
		AccessedObject:    accessedObject,
		OuterNode:         outerNode,
		ReplacementStart:  replacementStart,
		ReplacementLength: replacementLength,
	}
}

// ParseDataForExtendsClassCompletion parses position context for a completion in the
// extends clause of a class declaration. It calls ParseAccessedExpressionForCompletion
// and then walks up the AST to find the enclosing ClassDeclaration.
func ParseDataForExtendsClassCompletion(sf *ast.SourceFile, position int) *ExtendsClassCompletionData {
	result := ParseAccessedExpressionForCompletion(sf, position)
	if result == nil {
		return nil
	}

	if !ast.IsIdentifier(result.AccessedObject) {
		return nil
	}

	// Walk up from OuterNode.parent through ExpressionWithTypeArguments and HeritageClause
	classDecl := result.OuterNode.Parent
	for classDecl != nil &&
		(ast.IsExpressionWithTypeArguments(classDecl) || ast.IsHeritageClause(classDecl)) {
		if classDecl.Parent == nil {
			break
		}
		classDecl = classDecl.Parent
	}

	if classDecl == nil || !ast.IsClassDeclaration(classDecl) {
		return nil
	}

	className := classDecl.Name()
	if className == nil {
		return nil
	}

	return &ExtendsClassCompletionData{
		AccessedObject:    result.AccessedObject,
		ClassDeclaration:  classDecl,
		ClassName:         className,
		ReplacementStart:  result.ReplacementStart,
		ReplacementLength: result.ReplacementLength,
	}
}

// AccessedObjectText returns the text of the accessed object node.
func (d *ExtendsClassCompletionData) AccessedObjectText() string {
	return scanner.GetTextOfNode(d.AccessedObject)
}

// ClassNameText returns the text of the class name node.
func (d *ExtendsClassCompletionData) ClassNameText() string {
	return scanner.GetTextOfNode(d.ClassName)
}
