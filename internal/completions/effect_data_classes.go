package completions

import (
	"fmt"

	"github.com/effect-ts/effect-typescript-go/internal/completion"
	"github.com/effect-ts/effect-typescript-go/internal/effectutil"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// effectDataClasses provides completion items for Data.TaggedError and Data.TaggedClass
// when the cursor is in the extends clause of a class declaration.
// For example, typing `class Foo extends Data.|` will suggest `TaggedError("Foo")` and `TaggedClass("Foo")`.
var effectDataClasses = completion.Completion{
	Name:        "effectDataClasses",
	Description: "Provides Data.TaggedError and Data.TaggedClass completions in extends clauses",
	Run:         runEffectDataClasses,
}

func runEffectDataClasses(ctx *completion.Context) []*lsproto.CompletionItem {
	data := completion.ParseDataForExtendsClassCompletion(ctx.SourceFile, ctx.Position)
	if data == nil {
		return nil
	}

	dataIdentifier := effectutil.FindModuleIdentifier(ctx.SourceFile, "Data")
	accessedText := data.AccessedObjectText()
	isFullyQualified := dataIdentifier == accessedText
	className := data.ClassNameText()

	// Get checker for API reference checks
	ch, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	defer done()

	// Build replacement range from byte offsets
	replacementRange := byteSpanToRange(ctx, data.ReplacementStart, data.ReplacementLength)

	sortText := "11"
	var items []*lsproto.CompletionItem

	// Data.TaggedError
	if isFullyQualified || typeparser.IsNodeReferenceToEffectDataModuleApi(ch, data.AccessedObject, "TaggedError") {
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.TaggedError("%s")<{${0}}>{}`, dataIdentifier, className)
		} else {
			insertText = fmt.Sprintf(`TaggedError("%s")<{${0}}>{}`, className)
		}
		items = append(items, makeCompletionItem(
			fmt.Sprintf(`TaggedError("%s")`, className),
			insertText, sortText, replacementRange,
		))
	}

	// Data.TaggedClass
	if isFullyQualified || typeparser.IsNodeReferenceToEffectDataModuleApi(ch, data.AccessedObject, "TaggedClass") {
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.TaggedClass("%s")<{${0}}>{}`, dataIdentifier, className)
		} else {
			insertText = fmt.Sprintf(`TaggedClass("%s")<{${0}}>{}`, className)
		}
		items = append(items, makeCompletionItem(
			fmt.Sprintf(`TaggedClass("%s")`, className),
			insertText, sortText, replacementRange,
		))
	}

	return items
}
