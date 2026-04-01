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

// SchemaUnionOfLiterals detects Schema.Union(...) calls where all arguments
// are Schema.Literal(...) calls and suggests combining them into a single
// Schema.Literal call. This rule is V3-only and disabled by default.
var SchemaUnionOfLiterals = rule.Rule{
	Name:            "schemaUnionOfLiterals",
	Group:           "style",
	Description:     "Suggests combining multiple Schema.Literal calls in Schema.Union into a single Schema.Literal",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3"},
	Codes:           []int32{tsdiag.This_Schema_Union_contains_multiple_Schema_Literal_members_and_can_be_simplified_to_a_single_Schema_Literal_call_effect_schemaUnionOfLiterals.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeSchemaUnionOfLiterals(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.This_Schema_Union_contains_multiple_Schema_Literal_members_and_can_be_simplified_to_a_single_Schema_Literal_call_effect_schemaUnionOfLiterals, nil)
		}
		return diags
	},
}

// SchemaUnionOfLiteralsMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the schemaUnionOfLiterals pattern.
type SchemaUnionOfLiteralsMatch struct {
	SourceFile             *ast.SourceFile
	Location               core.TextRange // Pre-computed error range for the diagnostic
	UnionCallNode          *ast.Node      // The full Schema.Union(...) call expression to be replaced
	FirstLiteralExpression *ast.Node      // The callee expression (Schema.Literal) from the first argument
	AllLiteralArgs         []*ast.Node    // All arguments collected from every Schema.Literal(...) call, in order
}

// AnalyzeSchemaUnionOfLiterals finds all Schema.Union(...) calls where every argument
// is a Schema.Literal(...) call, returning matches with captured nodes for diagnostics and fixes.
func AnalyzeSchemaUnionOfLiterals(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []SchemaUnionOfLiteralsMatch {
	// V3-only rule
	if tp.SupportedEffectVersion() != typeparser.EffectMajorV3 {
		return nil
	}

	var matches []SchemaUnionOfLiteralsMatch

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
			if m, ok := analyzeSchemaUnionOfLiteralsNode(tp, c, sf, node); ok {
				matches = append(matches, m)
			}
		}

		node.ForEachChild(pushChild)
	}

	return matches
}

// analyzeSchemaUnionOfLiteralsNode checks if a call expression is Schema.Union(...)
// where all arguments are Schema.Literal(...) calls.
func analyzeSchemaUnionOfLiteralsNode(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile, node *ast.Node) (SchemaUnionOfLiteralsMatch, bool) {
	call := node.AsCallExpression()

	// Check if this is Schema.Union
	if !tp.IsNodeReferenceToEffectSchemaModuleApi(call.Expression, "Union") {
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
		if !tp.IsNodeReferenceToEffectSchemaModuleApi(argCall.Expression, "Literal") {
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
		SourceFile:             sf,
		Location:               scanner.GetErrorRangeForNode(sf, node),
		UnionCallNode:          node,
		FirstLiteralExpression: firstLiteralExpression,
		AllLiteralArgs:         allLiteralArgs,
	}, true
}
