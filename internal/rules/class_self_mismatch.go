package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// ClassSelfMismatch ensures the Self type parameter matches the class name in
// Effect.Service, Context.Tag, Effect.Tag, Schema.Class, Schema.TaggedClass,
// Schema.TaggedError, Schema.TaggedRequest, Schema.RequestClass, and Model.Class declarations.
var ClassSelfMismatch = rule.Rule{
	Name:            "classSelfMismatch",
	Description:     "Ensures Self type parameter matches the class name in Service/Tag/Schema classes",
	DefaultSeverity: etscore.SeverityError,
	Codes:           []int32{tsdiag.Self_type_parameter_should_be_0_effect_classSelfMismatch.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeClassSelfMismatch(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.Location, tsdiag.Self_type_parameter_should_be_0_effect_classSelfMismatch, nil, m.ExpectedName)
		}
		return diags
	},
}

// ClassSelfMismatchMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the classSelfMismatch pattern.
type ClassSelfMismatchMatch struct {
	Location     core.TextRange // The pre-computed error range for the selfTypeNode
	SelfTypeNode *ast.Node      // The Self type argument node
	ClassName    *ast.Node      // The class name identifier
	ExpectedName string         // The expected name (from the class declaration)
	ActualName   string         // The actual name found in the Self type parameter
}

// AnalyzeClassSelfMismatch finds all class declarations where the Self type
// parameter does not match the class name.
func AnalyzeClassSelfMismatch(c *checker.Checker, sf *ast.SourceFile) []ClassSelfMismatchMatch {
	var matches []ClassSelfMismatchMatch

	nodeToVisit := make([]*ast.Node, 0)
	for child := range sf.AsNode().IterChildren() {
		nodeToVisit = append(nodeToVisit, child)
	}

	for len(nodeToVisit) > 0 {
		node := nodeToVisit[len(nodeToVisit)-1]
		nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

		if node.Kind == ast.KindClassDeclaration && node.Name() != nil {
			if m := checkClassSelfMismatch(c, sf, node); m != nil {
				matches = append(matches, *m)
			}
		}

		for child := range node.IterChildren() {
			nodeToVisit = append(nodeToVisit, child)
		}
	}

	return matches
}

func checkClassSelfMismatch(c *checker.Checker, sf *ast.SourceFile, classNode *ast.Node) *ClassSelfMismatchMatch {
	var selfTypeNode *ast.Node
	var className *ast.Node

	// Try extends* functions in order, matching the TS reference
	if result := typeparser.ExtendsEffectService(c, classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := typeparser.ExtendsServiceMapService(c, classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := typeparser.ExtendsContextTag(c, classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := typeparser.ExtendsEffectTag(c, classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := typeparser.ExtendsSchemaClass(c, classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := typeparser.ExtendsSchemaTaggedClass(c, classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := typeparser.ExtendsSchemaTaggedError(c, classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := typeparser.ExtendsSchemaTaggedRequest(c, classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := typeparser.ExtendsSchemaRequestClass(c, classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := typeparser.ExtendsEffectSqlModelClass(c, classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := typeparser.ExtendsEffectModelClass(c, classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	}

	if selfTypeNode == nil || className == nil {
		return nil
	}

	// Extract actual name from the Self type node
	actualName := extractSelfTypeName(sf, selfTypeNode)

	// Get expected name from the class name
	expectedName := scanner.GetTextOfNode(className)

	if actualName == expectedName {
		return nil
	}

	return &ClassSelfMismatchMatch{
		Location:     scanner.GetErrorRangeForNode(sf, selfTypeNode),
		SelfTypeNode: selfTypeNode,
		ClassName:    className,
		ExpectedName: expectedName,
		ActualName:   actualName,
	}
}

// extractSelfTypeName extracts the name text from a Self type node.
// For TypeReferenceNode with Identifier typeName → identifier text.
// For TypeReferenceNode with QualifiedName typeName → right identifier text.
// Fallback → source text substring between node pos and end.
func extractSelfTypeName(sf *ast.SourceFile, selfTypeNode *ast.Node) string {
	if ast.IsTypeReferenceNode(selfTypeNode) {
		typeName := selfTypeNode.AsTypeReferenceNode().TypeName
		if ast.IsIdentifier(typeName) {
			return typeName.Text()
		}
		if ast.IsQualifiedName(typeName) {
			return typeName.AsQualifiedName().Right.Text()
		}
	}
	// Fallback: use source text
	text := sf.Text()
	pos := selfTypeNode.Pos()
	end := selfTypeNode.End()
	if pos >= 0 && end >= pos && end <= len(text) {
		return text[pos:end]
	}
	return ""
}
