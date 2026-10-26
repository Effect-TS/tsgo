package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var PreferSchemaTypePropertyFix = fixable.Fixable{
	Name:        "preferSchemaTypeProperty",
	Description: "Replace with typeof X.Type or typeof X.Encoded",
	ErrorCodes:  []int32{tsdiag.Do_not_use_0_typeof_X_Use_typeof_X_1_instead_effect_preferSchemaTypeProperty.Code()},
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
			Description: "Replace with 'typeof " + match.SchemaNameText + "." + match.PropertyName + "'",
			Run: func(tracker *rewriter.Tracker) {
				schemaType := tracker.NewQualifiedName(
					tracker.DeepCloneNode(match.SchemaName),
					tracker.NewIdentifier(match.PropertyName),
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
