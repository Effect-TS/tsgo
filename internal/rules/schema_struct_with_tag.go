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

// SchemaStructWithTag detects Schema.Struct(...) calls where the argument
// contains a _tag property assigned to Schema.Literal("someString"), and
// suggests using Schema.TaggedStruct instead.
var SchemaStructWithTag = rule.Rule{
	Name:            "schemaStructWithTag",
	Group:           "style",
	Description:     "Suggests using Schema.TaggedStruct instead of Schema.Struct with _tag field",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Schema_Struct_with_a_tag_field_can_be_simplified_to_Schema_TaggedStruct_to_make_the_tag_optional_in_the_constructor_effect_schemaStructWithTag.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeSchemaStructWithTag(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Schema_Struct_with_a_tag_field_can_be_simplified_to_Schema_TaggedStruct_to_make_the_tag_optional_in_the_constructor_effect_schemaStructWithTag, nil)
		}
		return diags
	},
}

// SchemaStructWithTagMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the schemaStructWithTag pattern.
type SchemaStructWithTagMatch struct {
	SourceFile      *ast.SourceFile
	Location        core.TextRange // Pre-computed error range for the diagnostic
	CallNode        *ast.Node      // The full Schema.Struct(...) call expression
	TagValue        string         // The string value extracted from Schema.Literal("...")
	OtherProperties []*ast.Node    // All property assignment nodes in the object literal except _tag
	SchemaExpr      *ast.Node      // The expression node for the Schema part of Schema.Struct
}

// AnalyzeSchemaStructWithTag finds all Schema.Struct({ _tag: Schema.Literal("..."), ... })
// call expressions and returns match structs for each.
func AnalyzeSchemaStructWithTag(c *checker.Checker, sf *ast.SourceFile) []SchemaStructWithTagMatch {
	var matches []SchemaStructWithTagMatch

	// Stack-based traversal
	nodeToVisit := make([]*ast.Node, 0)
	pushChild := func(child *ast.Node) bool {
		nodeToVisit = append(nodeToVisit, child)
		return false
	}
	sf.AsNode().ForEachChild(pushChild)

	for len(nodeToVisit) > 0 {
		node := nodeToVisit[len(nodeToVisit)-1]
		nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

		if node.Kind == ast.KindCallExpression {
			if m, ok := analyzeSchemaStructWithTagNode(c, sf, node); ok {
				matches = append(matches, m)
			}
		}

		// Enqueue children
		node.ForEachChild(pushChild)
	}

	return matches
}

// analyzeSchemaStructWithTagNode checks a call expression for Schema.Struct({ _tag: Schema.Literal("...") }).
func analyzeSchemaStructWithTagNode(c *checker.Checker, sf *ast.SourceFile, node *ast.Node) (SchemaStructWithTagMatch, bool) {
	if node.Kind != ast.KindCallExpression {
		return SchemaStructWithTagMatch{}, false
	}
	call := node.AsCallExpression()

	// Check if this is Schema.Struct
	if !typeparser.IsNodeReferenceToEffectSchemaModuleApi(c, call.Expression, "Struct") {
		return SchemaStructWithTagMatch{}, false
	}

	// Must have exactly 1 argument
	if call.Arguments == nil || len(call.Arguments.Nodes) != 1 {
		return SchemaStructWithTagMatch{}, false
	}

	arg := call.Arguments.Nodes[0]
	if arg == nil || arg.Kind != ast.KindObjectLiteralExpression {
		return SchemaStructWithTagMatch{}, false
	}

	objLit := arg.AsObjectLiteralExpression()
	if objLit == nil || objLit.Properties == nil {
		return SchemaStructWithTagMatch{}, false
	}

	// Look for a _tag property assignment
	for _, prop := range objLit.Properties.Nodes {
		if prop == nil || prop.Kind != ast.KindPropertyAssignment {
			continue
		}
		pa := prop.AsPropertyAssignment()
		if pa == nil || pa.Name() == nil {
			continue
		}
		if pa.Name().Kind != ast.KindIdentifier || scanner.GetTextOfNode(pa.Name()) != "_tag" {
			continue
		}

		// Found _tag property — check if its initializer is Schema.Literal("...")
		init := pa.Initializer
		if init == nil || init.Kind != ast.KindCallExpression {
			return SchemaStructWithTagMatch{}, false
		}

		literalCall := init.AsCallExpression()
		if !typeparser.IsNodeReferenceToEffectSchemaModuleApi(c, literalCall.Expression, "Literal") {
			return SchemaStructWithTagMatch{}, false
		}

		// Schema.Literal must have exactly 1 argument that is a string literal
		if literalCall.Arguments == nil || len(literalCall.Arguments.Nodes) != 1 {
			return SchemaStructWithTagMatch{}, false
		}
		if literalCall.Arguments.Nodes[0].Kind != ast.KindStringLiteral {
			return SchemaStructWithTagMatch{}, false
		}

		tagValue := literalCall.Arguments.Nodes[0].AsStringLiteral().Text

		// Collect all properties except _tag
		var otherProps []*ast.Node
		for _, p := range objLit.Properties.Nodes {
			if p == prop {
				continue
			}
			otherProps = append(otherProps, p)
		}

		// Extract the Schema expression from Schema.Struct (the left side of the property access)
		var schemaExpr *ast.Node
		if call.Expression.Kind == ast.KindPropertyAccessExpression {
			schemaExpr = call.Expression.AsPropertyAccessExpression().Expression
		}

		return SchemaStructWithTagMatch{
			SourceFile:      sf,
			Location:        scanner.GetErrorRangeForNode(sf, node),
			CallNode:        node,
			TagValue:        tagValue,
			OtherProperties: otherProps,
			SchemaExpr:      schemaExpr,
		}, true
	}

	return SchemaStructWithTagMatch{}, false
}
