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

// SchemaUnionOfLiterals detects Schema.Union(...) calls where all arguments
// are Schema.Literal(...) calls and suggests combining them into a single
// Schema.Literal call. This rule is V3-only and disabled by default.
var SchemaUnionOfLiterals = rule.Rule{
	Name:            "schemaUnionOfLiterals",
	Description:     "Suggests combining multiple Schema.Literal calls in Schema.Union into a single Schema.Literal",
	DefaultSeverity: etscore.SeverityOff,
	Codes:           []int32{tsdiag.A_Schema_Union_of_multiple_Schema_Literal_calls_can_be_simplified_to_a_single_Schema_Literal_call_effect_schemaUnionOfLiterals.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeSchemaUnionOfLiterals(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.Location, tsdiag.A_Schema_Union_of_multiple_Schema_Literal_calls_can_be_simplified_to_a_single_Schema_Literal_call_effect_schemaUnionOfLiterals, nil)
		}
		return diags
	},
}

// SchemaUnionOfLiteralsMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the schemaUnionOfLiterals pattern.
type SchemaUnionOfLiteralsMatch struct {
	Location               core.TextRange // Pre-computed error range for the diagnostic
	UnionCallNode          *ast.Node      // The full Schema.Union(...) call expression to be replaced
	FirstLiteralExpression *ast.Node      // The callee expression (Schema.Literal) from the first argument
	AllLiteralArgs         []*ast.Node    // All arguments collected from every Schema.Literal(...) call, in order
}

// AnalyzeSchemaUnionOfLiterals finds all Schema.Union(...) calls where every argument
// is a Schema.Literal(...) call, returning matches with captured nodes for diagnostics and fixes.
func AnalyzeSchemaUnionOfLiterals(c *checker.Checker, sf *ast.SourceFile) []SchemaUnionOfLiteralsMatch {
	// V3-only rule
	if typeparser.SupportedEffectVersion(c) != typeparser.EffectMajorV3 {
		return nil
	}

	var matches []SchemaUnionOfLiteralsMatch

	nodeToVisit := make([]*ast.Node, 0)
	for child := range sf.AsNode().IterChildren() {
		nodeToVisit = append(nodeToVisit, child)
	}

	for len(nodeToVisit) > 0 {
		node := nodeToVisit[len(nodeToVisit)-1]
		nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

		if node.Kind == ast.KindCallExpression {
			if m, ok := analyzeSchemaUnionOfLiteralsNode(c, sf, node); ok {
				matches = append(matches, m)
			}
		}

		for child := range node.IterChildren() {
			nodeToVisit = append(nodeToVisit, child)
		}
	}

	return matches
}

// analyzeSchemaUnionOfLiteralsNode checks if a call expression is Schema.Union(...)
// where all arguments are Schema.Literal(...) calls.
func analyzeSchemaUnionOfLiteralsNode(c *checker.Checker, sf *ast.SourceFile, node *ast.Node) (SchemaUnionOfLiteralsMatch, bool) {
	call := node.AsCallExpression()

	// Check if this is Schema.Union
	if !typeparser.IsNodeReferenceToEffectSchemaModuleApi(c, call.Expression, "Union") {
		return SchemaUnionOfLiteralsMatch{}, false
	}

	// Must have at least 2 arguments
	if call.Arguments == nil || len(call.Arguments.Nodes) < 2 {
		return SchemaUnionOfLiteralsMatch{}, false
	}

	var firstLiteralExpression *ast.Node
	var allLiteralArgs []*ast.Node

	// Check that every argument is a call expression referencing Schema.Literal
	for i, arg := range call.Arguments.Nodes {
		if arg == nil || arg.Kind != ast.KindCallExpression {
			return SchemaUnionOfLiteralsMatch{}, false
		}
		argCall := arg.AsCallExpression()
		if !typeparser.IsNodeReferenceToEffectSchemaModuleApi(c, argCall.Expression, "Literal") {
			return SchemaUnionOfLiteralsMatch{}, false
		}

		if i == 0 {
			firstLiteralExpression = argCall.Expression
		}

		// Collect all arguments from this Schema.Literal(...) call
		if argCall.Arguments != nil {
			allLiteralArgs = append(allLiteralArgs, argCall.Arguments.Nodes...)
		}
	}

	return SchemaUnionOfLiteralsMatch{
		Location:               scanner.GetErrorRangeForNode(sf, node),
		UnionCallNode:          node,
		FirstLiteralExpression: firstLiteralExpression,
		AllLiteralArgs:         allLiteralArgs,
	}, true
}
