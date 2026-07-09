package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

var SchemaNumberFix = fixable.Fixable{
	Name:        "schemaNumber",
	Description: "Replace Schema.Number APIs with finite variants",
	ErrorCodes:  []int32{tsdiag.This_Schema_number_API_accepts_NaN_Infinity_and_Infinity_Use_0_for_finite_domain_numbers_If_non_finite_values_are_intentional_disable_this_diagnostic_for_that_line_effect_schemaNumber.Code()},
	FixIDs:      []string{"schemaNumber_fix"},
	Run:         runSchemaNumberFix,
}

func runSchemaNumberFix(ctx *fixable.Context) []ls.CodeAction {
	matches := rules.AnalyzeSchemaNumber(ctx.TypeParser, ctx.SourceFile)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.ReferenceNode == nil || match.ReferenceNode.Kind != ast.KindIdentifier {
			continue
		}
		if match.ReferenceNode.Parent == nil || match.ReferenceNode.Parent.Kind != ast.KindPropertyAccessExpression {
			continue
		}

		referenceNode := match.ReferenceNode
		replacement := match.ReplacementIdentifier
		description := "Replace with " + match.Replacement
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: description,
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceRangeWithText(ctx.SourceFile, lsproto.Range{
					Start: ctx.BytePosToLSPPosition(referenceNode.Pos()),
					End:   ctx.BytePosToLSPPosition(referenceNode.End()),
				}, replacement)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
