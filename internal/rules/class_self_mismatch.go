package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// ClassSelfMismatch ensures the Self type parameter matches the class name in
// Effect.Service, ServiceMap.Service, Context.Tag, Effect.Tag, Schema.Class,
// Schema.TaggedClass, Schema.TaggedError, Schema.TaggedRequest,
// Schema.RequestClass, and Model.Class declarations.
var ClassSelfMismatch = rule.Rule{
	Name:            "classSelfMismatch",
	Group:           "correctness",
	Description:     "Ensures Self type parameter matches the class name in ServiceMap/Service/Tag/Schema classes",
	DefaultSeverity: etscore.SeverityError,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Self_type_parameter_should_be_0_effect_classSelfMismatch.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeClassSelfMismatch(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Self_type_parameter_should_be_0_effect_classSelfMismatch, nil, m.ExpectedName)
		}
		return diags
	},
}

// ClassSelfMismatchMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the classSelfMismatch pattern.
type ClassSelfMismatchMatch struct {
	SourceFile   *ast.SourceFile // The source file of the match
	Location     core.TextRange  // The pre-computed error range for the selfTypeNode
	SelfTypeNode *ast.Node       // The Self type argument node
	ClassName    *ast.Node       // The class name identifier
	ExpectedName string          // The expected name (from the class declaration)
	ActualName   string          // The actual name found in the Self type parameter
}

// AnalyzeClassSelfMismatch finds all class declarations where the Self type
// parameter does not match the class name.
func AnalyzeClassSelfMismatch(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []ClassSelfMismatchMatch {
	var matches []ClassSelfMismatchMatch

	nodeToVisit := make([]*ast.Node, 0)
	pushChild := func(child *ast.Node) bool {
		nodeToVisit = append(nodeToVisit, child)
		return false
	}
	sf.AsNode().ForEachChild(pushChild)

	for len(nodeToVisit) > 0 {
		node := nodeToVisit[len(nodeToVisit)-1]
		nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

		if node.Kind == ast.KindClassDeclaration && node.Name() != nil {
			if m := checkClassSelfMismatch(tp, c, sf, node); m != nil {
				matches = append(matches, *m)
			}
		}

		node.ForEachChild(pushChild)
	}

	return matches
}

func checkClassSelfMismatch(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile, classNode *ast.Node) *ClassSelfMismatchMatch {
	var selfTypeNode *ast.Node
	var className *ast.Node

	// Try extends* functions in order, matching the TS reference
	if result := tp.ExtendsEffectService(classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := tp.ExtendsServiceMapService(classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := tp.ExtendsContextTag(classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := tp.ExtendsEffectTag(classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := tp.ExtendsSchemaClass(classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := tp.ExtendsSchemaTaggedClass(classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := tp.ExtendsSchemaTaggedError(classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := tp.ExtendsSchemaTaggedRequest(classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := tp.ExtendsSchemaRequestClass(classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := tp.ExtendsEffectSqlModelClass(classNode); result != nil {
		selfTypeNode = result.SelfTypeNode
		className = result.ClassName
	} else if result := tp.ExtendsEffectModelClass(classNode); result != nil {
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
		SourceFile:   sf,
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
