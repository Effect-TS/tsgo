package completions

import (
	"fmt"

	"github.com/effect-ts/tsgo/internal/completion"
	"github.com/effect-ts/tsgo/internal/effectutil"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// effectSqlModelSelfInClasses provides completion items for @effect/sql Model.Class
// when the cursor is in the extends clause of a class declaration.
// This is a V3-only completion.
var effectSqlModelSelfInClasses = completion.Completion{
	Name:        "effectSqlModelSelfInClasses",
	Description: "Provides @effect/sql Model.Class completions in extends clauses",
	Run:         runEffectSqlModelSelfInClasses,
}

func runEffectSqlModelSelfInClasses(ctx *completion.Context) []*lsproto.CompletionItem {
	data := completion.ParseDataForExtendsClassCompletion(ctx.SourceFile, ctx.Position)
	if data == nil {
		return nil
	}

	// Get checker for version detection and API reference checks
	ch, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	defer done()

	// V3 only
	version := typeparser.SupportedEffectVersion(ch)
	if version != typeparser.EffectMajorV3 {
		return nil
	}

	modelIdentifier := effectutil.FindModuleIdentifierForPackage(ctx.SourceFile, "@effect/sql", "Model")
	accessedText := data.AccessedObjectText()
	isFullyQualified := modelIdentifier == accessedText
	className := data.ClassNameText()

	// For non-fully-qualified: validate with IsNodeReferenceToEffectSqlModelModuleApi
	if !isFullyQualified && !typeparser.IsNodeReferenceToEffectSqlModelModuleApi(ch, data.AccessedObject, "Class") {
		return nil
	}

	// Build replacement range from byte offsets
	replacementRange := byteSpanToRange(ctx, data.ReplacementStart, data.ReplacementLength)

	sortText := "11"

	var insertText string
	if isFullyQualified {
		insertText = fmt.Sprintf(`%s.Class<%s>("%s")({${0}}){}`, modelIdentifier, className, className)
	} else {
		insertText = fmt.Sprintf(`Class<%s>("%s")({${0}}){}`, className, className)
	}

	return []*lsproto.CompletionItem{
		makeExtendsCompletionItem(accessedText,
			fmt.Sprintf("Class<%s>", className),
			insertText, sortText, replacementRange,
		),
	}
}
