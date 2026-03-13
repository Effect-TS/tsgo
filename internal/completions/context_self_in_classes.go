package completions

import (
	"fmt"

	"github.com/effect-ts/effect-typescript-go/internal/completion"
	"github.com/effect-ts/effect-typescript-go/internal/effectutil"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// contextSelfInClasses provides completion items for Context.Tag class constructors
// when the cursor is in the extends clause of a class declaration.
// This is a V3-only completion.
var contextSelfInClasses = completion.Completion{
	Name:        "contextSelfInClasses",
	Description: "Provides Context.Tag completions in extends clauses",
	Run:         runContextSelfInClasses,
}

func runContextSelfInClasses(ctx *completion.Context) []*lsproto.CompletionItem {
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

	contextIdentifier := effectutil.FindModuleIdentifier(ctx.SourceFile, "Context")
	accessedText := data.AccessedObjectText()
	isFullyQualified := contextIdentifier == accessedText
	className := data.ClassNameText()

	// For non-fully-qualified: validate with IsNodeReferenceToEffectContextModuleApi
	if !isFullyQualified && !typeparser.IsNodeReferenceToEffectContextModuleApi(ch, data.AccessedObject, "Tag") {
		return nil
	}

	// Compute deterministic tag key
	tagKey := computeServiceTagKey(ch, ctx.SourceFile, className)

	// Build replacement range from byte offsets
	replacementRange := byteSpanToRange(ctx, data.ReplacementStart, data.ReplacementLength)

	sortText := "11"

	var insertText string
	if isFullyQualified {
		insertText = fmt.Sprintf(`%s.Tag("%s")<%s, ${0}>(){}`, contextIdentifier, tagKey, className)
	} else {
		insertText = fmt.Sprintf(`Tag("%s")<%s, ${0}>(){}`, tagKey, className)
	}

	return []*lsproto.CompletionItem{
		makeCompletionItem(
			fmt.Sprintf(`Tag("%s")`, className),
			insertText, sortText, replacementRange,
		),
	}
}
