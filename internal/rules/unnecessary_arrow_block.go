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

// UnnecessaryArrowBlock suggests using a concise arrow body when a block only
// contains a single return statement.
var UnnecessaryArrowBlock = rule.Rule{
	Name:            "unnecessaryArrowBlock",
	Group:           "style",
	Description:     "Suggests using a concise arrow body when the block only returns an expression",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.This_arrow_function_block_only_returns_an_expression_and_can_use_a_concise_body_effect_unnecessaryArrowBlock.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeUnnecessaryArrowBlock(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.This_arrow_function_block_only_returns_an_expression_and_can_use_a_concise_body_effect_unnecessaryArrowBlock,
				nil,
			)
		}
		return diags
	},
}

type UnnecessaryArrowBlockMatch struct {
	SourceFile         *ast.SourceFile
	Location           core.TextRange
	ArrowFunction      *ast.Node
	BodyBlock          *ast.Node
	ReturnedExpression *ast.Node
}

func AnalyzeUnnecessaryArrowBlock(_ *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []UnnecessaryArrowBlockMatch {
	var matches []UnnecessaryArrowBlockMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindArrowFunction {
			if match := analyzeUnnecessaryArrowBlockNode(sf, n); match != nil {
				matches = append(matches, *match)
			}
		}

		n.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}

func analyzeUnnecessaryArrowBlockNode(sf *ast.SourceFile, n *ast.Node) *UnnecessaryArrowBlockMatch {
	arrowFn := n.AsArrowFunction()
	if arrowFn == nil || arrowFn.Body == nil || arrowFn.Body.Kind != ast.KindBlock {
		return nil
	}

	body := arrowFn.Body.AsBlock()
	if body.Statements == nil || len(body.Statements.Nodes) != 1 {
		return nil
	}

	stmt := body.Statements.Nodes[0]
	if stmt == nil || stmt.Kind != ast.KindReturnStatement {
		return nil
	}

	returnedExpression := stmt.AsReturnStatement().Expression
	if returnedExpression == nil {
		return nil
	}

	return &UnnecessaryArrowBlockMatch{
		SourceFile:         sf,
		Location:           scanner.GetErrorRangeForNode(sf, arrowFn.Body),
		ArrowFunction:      n,
		BodyBlock:          arrowFn.Body,
		ReturnedExpression: returnedExpression,
	}
}
