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

// InstanceOfSchema suggests using Schema.is instead of instanceof for Effect Schema types.
// This rule is disabled by default.
var InstanceOfSchema = rule.Rule{
	Name:            "instanceOfSchema",
	Group:           "effectNative",
	Description:     "Suggests using Schema.is instead of instanceof for Effect Schema types",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Consider_using_Schema_is_instead_of_instanceof_for_Effect_Schema_types_effect_instanceOfSchema.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeInstanceOfSchema(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Consider_using_Schema_is_instead_of_instanceof_for_Effect_Schema_types_effect_instanceOfSchema, nil)
		}
		return diags
	},
}

// InstanceOfSchemaMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the instanceOfSchema pattern.
type InstanceOfSchemaMatch struct {
	SourceFile     *ast.SourceFile
	Location       core.TextRange // The pre-computed error range for this match
	InstanceOfNode *ast.Node      // The full BinaryExpression node (the instanceof expression)
	LeftExpr       *ast.Node      // Left operand of instanceof (the value being tested)
	RightExpr      *ast.Node      // Right operand of instanceof (the schema type reference)
}

// AnalyzeInstanceOfSchema finds all `value instanceof SchemaClass` expressions
// where the right-hand side is an Effect Schema type.
func AnalyzeInstanceOfSchema(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []InstanceOfSchemaMatch {
	var matches []InstanceOfSchemaMatch

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

		if ast.IsInstanceOfExpression(node) {
			binExpr := node.AsBinaryExpression()
			rightExpr := binExpr.Right
			rightType := tp.GetTypeAtLocation(rightExpr)
			if rightType != nil && tp.IsSchemaType(rightType, rightExpr) {
				matches = append(matches, InstanceOfSchemaMatch{
					SourceFile:     sf,
					Location:       scanner.GetErrorRangeForNode(sf, node),
					InstanceOfNode: node,
					LeftExpr:       binExpr.Left,
					RightExpr:      rightExpr,
				})
			}
		}

		// Enqueue children
		node.ForEachChild(pushChild)
	}

	return matches
}
