package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
)

var PreferSchemaTypePropertyFix = fixable.Fixable{
	Name:        "preferSchemaTypeProperty",
	Description: "Replace with typeof X.Type",
	ErrorCodes:  []int32{tsdiag.Do_not_use_Schema_Schema_Type_typeof_X_to_extract_a_schema_s_type_Use_typeof_X_Type_instead_effect_preferSchemaTypeProperty.Code()},
	FixIDs:      []string{"preferSchemaTypeProperty_fix"},
	Run:         runPreferSchemaTypePropertyFix,
}

func runPreferSchemaTypePropertyFix(ctx *fixable.Context) []ls.CodeAction {
	matches := rules.AnalyzePreferSchemaTypeProperty(ctx.TypeParser, ctx.SourceFile)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with 'typeof " + match.SchemaNameText + ".Type'",
			Run: func(tracker *rewriter.Tracker) {
				schemaType := tracker.NewQualifiedName(
					tracker.DeepCloneNode(match.SchemaName),
					tracker.NewIdentifier("Type"),
				)
				replacement := tracker.NewTypeQueryNode(schemaType, nil)
				tracker.ReplaceNode(ctx.SourceFile, match.TypeReference, replacement, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
