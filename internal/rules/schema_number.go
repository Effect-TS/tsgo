package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var SchemaNumber = rule.Rule{
	Name:            "schemaNumber",
	Group:           "style",
	Description:     "Suggests Schema.Finite and Schema.FiniteFromString instead of Schema.Number APIs when describing domain numbers",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.This_Schema_number_API_accepts_NaN_Infinity_and_Infinity_Use_0_for_finite_domain_numbers_If_non_finite_values_are_intentional_disable_this_diagnostic_for_that_line_effect_schemaNumber.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeSchemaNumber(ctx.TypeParser, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.This_Schema_number_API_accepts_NaN_Infinity_and_Infinity_Use_0_for_finite_domain_numbers_If_non_finite_values_are_intentional_disable_this_diagnostic_for_that_line_effect_schemaNumber, nil, m.Replacement)
		}
		return diags
	},
}

type SchemaNumberMatch struct {
	SourceFile            *ast.SourceFile
	Location              core.TextRange
	ReferenceNode         *ast.Node
	Replacement           string
	ReplacementIdentifier string
}

func AnalyzeSchemaNumber(tp *typeparser.TypeParser, sf *ast.SourceFile) []SchemaNumberMatch {
	if tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []SchemaNumberMatch
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		switch node.Kind {
		case ast.KindImportDeclaration, ast.KindImportEqualsDeclaration:
			return false
		case ast.KindPropertyAccessExpression:
			if match := analyzeSchemaNumberReference(tp, sf, node); match != nil {
				matches = append(matches, *match)
				return false
			}
		case ast.KindIdentifier:
			if match := analyzeSchemaNumberReference(tp, sf, node); match != nil {
				matches = append(matches, *match)
			}
		}

		node.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())

	flows := tp.PipingFlows(sf, false)
	filtered := matches[:0]
	for _, match := range matches {
		if schemaNumberHasFiniteMethodCheck(tp, match.ReferenceNode) || schemaNumberHasFinitePipingCheck(tp, match.ReferenceNode, flows) {
			continue
		}
		filtered = append(filtered, match)
	}
	return filtered
}

func analyzeSchemaNumberReference(tp *typeparser.TypeParser, sf *ast.SourceFile, node *ast.Node) *SchemaNumberMatch {
	for _, api := range schemaNumberApis {
		if tp.IsNodeReferenceToEffectSchemaModuleApi(node, api.Name) {
			referenceNode := schemaNumberReferenceLocation(node)
			return &SchemaNumberMatch{
				SourceFile:            sf,
				Location:              scanner.GetErrorRangeForNode(sf, referenceNode),
				ReferenceNode:         referenceNode,
				Replacement:           api.Replacement,
				ReplacementIdentifier: api.ReplacementIdentifier,
			}
		}
	}
	return nil
}

func schemaNumberHasFiniteMethodCheck(tp *typeparser.TypeParser, node *ast.Node) bool {
	schemaExpression := node
	if node.Kind == ast.KindIdentifier && node.Parent != nil && node.Parent.Kind == ast.KindPropertyAccessExpression {
		access := node.Parent.AsPropertyAccessExpression()
		if access.Name() == node {
			schemaExpression = node.Parent
		}
	}
	for schemaExpression.Parent != nil && schemaExpression.Parent.Kind == ast.KindPropertyAccessExpression {
		accessNode := schemaExpression.Parent
		access := accessNode.AsPropertyAccessExpression()
		if access.Expression != schemaExpression || access.Name() == nil || accessNode.Parent == nil || accessNode.Parent.Kind != ast.KindCallExpression {
			return false
		}

		call := accessNode.Parent.AsCallExpression()
		if call.Expression != accessNode {
			return false
		}
		switch access.Name().Text() {
		case "annotate":
			schemaExpression = accessNode.Parent
		case "check":
			if call.Arguments == nil {
				return false
			}
			return schemaNumberHasFinitePredicate(tp, call.Arguments.Nodes)
		default:
			return false
		}
	}
	return false
}

func schemaNumberHasFinitePipingCheck(tp *typeparser.TypeParser, reference *ast.Node, flows []*typeparser.PipingFlow) bool {
	for _, flow := range flows {
		if flow.Subject.Node == nil || !schemaNumberNodeContains(flow.Subject.Node, reference) {
			continue
		}
		for _, transformation := range flow.Transformations {
			if transformation.Callee != nil &&
				tp.IsNodeReferenceToEffectSchemaModuleApi(transformation.Callee, "check") &&
				schemaNumberHasFinitePredicate(tp, transformation.Args) {
				return true
			}
		}
	}
	return false
}

func schemaNumberHasFinitePredicate(tp *typeparser.TypeParser, arguments []*ast.Node) bool {
	for _, argument := range arguments {
		argument = ast.SkipParentheses(argument)
		if argument.Kind != ast.KindCallExpression {
			continue
		}
		predicateCall := argument.AsCallExpression()
		if predicateCall.Expression == nil {
			continue
		}
		if tp.IsNodeReferenceToEffectSchemaModuleApi(predicateCall.Expression, "isFinite") ||
			tp.IsNodeReferenceToEffectSchemaModuleApi(predicateCall.Expression, "isInt") {
			return true
		}
	}
	return false
}

func schemaNumberNodeContains(ancestor *ast.Node, target *ast.Node) bool {
	return ancestor != nil && target != nil && ancestor.Pos() <= target.Pos() && target.End() <= ancestor.End()
}

type schemaNumberApi struct {
	Name                  string
	Replacement           string
	ReplacementIdentifier string
}

var schemaNumberApis = []schemaNumberApi{
	{Name: "Number", Replacement: "Schema.Finite", ReplacementIdentifier: "Finite"},
	{Name: "NumberFromString", Replacement: "Schema.FiniteFromString", ReplacementIdentifier: "FiniteFromString"},
}

func schemaNumberReferenceLocation(node *ast.Node) *ast.Node {
	if node.Kind == ast.KindPropertyAccessExpression {
		if name := node.AsPropertyAccessExpression().Name(); name != nil {
			return name
		}
	}
	return node
}
