package completions

import (
	"fmt"

	"github.com/effect-ts/tsgo/internal/completion"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
)

// schemaBrand provides a `brand("varName")` completion when dot-accessing the
// Schema identifier inside a variable declaration. V3-only.
var schemaBrand = completion.Completion{
	Name:        "schemaBrand",
	Description: "Provides brand(\"varName\") completion when dot-accessing Schema in variable declarations",
	Run:         runSchemaBrand,
}

func runSchemaBrand(ctx *completion.Context) []*lsproto.CompletionItem {
	result := completion.ParseAccessedExpressionForCompletion(ctx.SourceFile, ctx.Position)
	if result == nil {
		return nil
	}

	// V3 only
	tp := ctx.TypeParser
	version := tp.SupportedEffectVersion()
	if version != typeparser.EffectMajorV3 {
		return nil
	}

	// Compare the accessed object with the file's reference to the Schema module
	if !typeparser.FindModuleReference(ctx.SourceFile, "Schema").MatchesNode(result.AccessedObject) {
		return nil
	}

	// Replacement span: from after the dot to the cursor position
	spanStart := result.AccessedObject.End() + 1
	spanLength := max(ctx.Position-spanStart, 0)

	// Find enclosing variable name
	varName := findEnclosingVariableName(result.OuterNode)
	if varName == "" {
		return nil
	}

	replacementRange := byteSpanToRange(ctx, spanStart, spanLength)
	sortText := "11"

	label := fmt.Sprintf(`brand("%s")`, varName)

	return []*lsproto.CompletionItem{
		makeCompletionItem(label, label, sortText, replacementRange),
	}
}
