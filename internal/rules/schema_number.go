package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
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
		case ast.KindPropertyAccessExpression:
			if match := analyzeSchemaNumberReference(tp, sf, node); match != nil {
				matches = append(matches, *match)
			}
		case ast.KindIdentifier:
			if !isSchemaNumberIdentifierReference(node) {
				break
			}
			if match := analyzeSchemaNumberReference(tp, sf, node); match != nil {
				matches = append(matches, *match)
			}
		}

		node.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
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

func isSchemaNumberIdentifierReference(node *ast.Node) bool {
	if node.Parent == nil {
		return true
	}

	switch node.Parent.Kind {
	case ast.KindPropertyAccessExpression:
		return node.Parent.AsPropertyAccessExpression().Name() != node
	case ast.KindImportSpecifier, ast.KindImportClause, ast.KindNamespaceImport:
		return false
	}

	return true
}
