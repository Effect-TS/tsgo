package completions

import (
	"fmt"

	"github.com/effect-ts/tsgo/internal/completion"
	"github.com/effect-ts/tsgo/internal/typeparser"
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

	dataIdentifier := typeparser.FindModuleIdentifier(ctx.SourceFile, "Data")
	accessedText := data.AccessedObjectText()
	isFullyQualified := dataIdentifier == accessedText
	className := data.ClassNameText()

	// Build replacement range from byte offsets
	replacementRange := byteSpanToRange(ctx, data.ReplacementStart, data.ReplacementLength)

	sortText := "11"
	var items []*lsproto.CompletionItem

	// Data.TaggedError
	if isFullyQualified || ctx.TypeParser.IsNodeReferenceToEffectDataModuleApi(data.AccessedObject, "TaggedError") {
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.TaggedError("%s")<{${0}}>{}`, dataIdentifier, className)
		} else {
			insertText = fmt.Sprintf(`TaggedError("%s")<{${0}}>{}`, className)
		}
		items = append(items, makeExtendsCompletionItem(accessedText,
			fmt.Sprintf(`TaggedError("%s")`, className),
			insertText, sortText, replacementRange,
		))
	}

	// Data.TaggedClass
	if isFullyQualified || ctx.TypeParser.IsNodeReferenceToEffectDataModuleApi(data.AccessedObject, "TaggedClass") {
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.TaggedClass("%s")<{${0}}>{}`, dataIdentifier, className)
		} else {
			insertText = fmt.Sprintf(`TaggedClass("%s")<{${0}}>{}`, className)
		}
		items = append(items, makeExtendsCompletionItem(accessedText,
			fmt.Sprintf(`TaggedClass("%s")`, className),
			insertText, sortText, replacementRange,
		))
	}

	return items
}
