package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
)

var PreferTypedSchemaDecoderFix = fixable.Fixable{
	Name:        "preferTypedSchemaDecoder",
	Description: "Replace the unknown-input Schema decoder with its typed variant",
	ErrorCodes: []int32{
		tsdiag.This_input_is_already_assignable_to_the_schema_s_Encoded_type_Use_0_to_preserve_compile_time_type_checking_instead_of_discarding_the_input_type_through_1_effect_preferTypedSchemaDecoder.Code(),
	},
	FixIDs: []string{"preferTypedSchemaDecoder_fix"},
	Run:    runPreferTypedSchemaDecoderFix,
}

func runPreferTypedSchemaDecoderFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzePreferTypedSchemaDecoder(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.DecoderName.Parent == nil || match.DecoderName.Parent.Kind != ast.KindPropertyAccessExpression {
			continue
		}
		m := match
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with " + m.TypedName,
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceNode(ctx.SourceFile, m.DecoderName, tracker.NewIdentifier(m.TypedName), nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
