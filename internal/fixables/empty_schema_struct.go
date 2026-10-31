package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var EmptySchemaStructFix = fixable.Fixable{
	Name:        "emptySchemaStruct",
	Description: "Replace with Tool.EmptyParams",
	ErrorCodes: []int32{
		tsdiag.Schema_Struct_as_tool_parameters_breaks_structured_output_schema_generation_for_some_providers_Use_Tool_EmptyParams_instead_effect_emptySchemaStruct.Code(),
	},
	FixIDs: []string{"emptySchemaStruct_fix"},
	Run:    runEmptySchemaStructFix,
}

func runEmptySchemaStructFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	for _, match := range rules.AnalyzeEmptySchemaStruct(ctx.TypeParser, ctx.Checker, sf) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		// Only the tool parameters case has a replacement; it goes through the
		// Tool module receiver of the constructor, which resolves where the edit lands.
		if !match.InToolParameters || match.ToolNamespace == nil {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with " + scanner.GetTextOfNode(match.ToolNamespace) + ".EmptyParams",
			Run: func(tracker *rewriter.Tracker) {
				emptyParams := tracker.NewPropertyAccessExpression(tracker.DeepCloneNode(match.ToolNamespace), nil, tracker.NewIdentifier("EmptyParams"), ast.NodeFlagsNone)
				tracker.ReplaceNode(sf, match.Call, emptyParams, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
