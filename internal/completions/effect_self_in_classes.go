package completions

import (
	"fmt"

	"github.com/effect-ts/effect-typescript-go/internal/completion"
	"github.com/effect-ts/effect-typescript-go/internal/effectutil"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// effectSelfInClasses provides completion items for Effect.Service and Effect.Tag class constructors
// when the cursor is in the extends clause of a class declaration.
// This is a V3-only completion.
var effectSelfInClasses = completion.Completion{
	Name:        "effectSelfInClasses",
	Description: "Provides Effect.Service/Effect.Tag completions in extends clauses",
	Run:         runEffectSelfInClasses,
}

func runEffectSelfInClasses(ctx *completion.Context) []*lsproto.CompletionItem {
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

	effectIdentifier := effectutil.FindModuleIdentifier(ctx.SourceFile, "Effect")
	accessedText := data.AccessedObjectText()
	isFullyQualified := effectIdentifier == accessedText
	className := data.ClassNameText()

	// Compute deterministic tag key
	tagKey := computeServiceTagKey(ch, ctx.SourceFile, className)

	// Build replacement range from byte offsets
	replacementRange := byteSpanToRange(ctx, data.ReplacementStart, data.ReplacementLength)

	sortText := "11"
	var items []*lsproto.CompletionItem

	// Service: Effect.Service<ClassName>()("tagKey", {}){}
	if isFullyQualified || typeparser.IsNodeReferenceToEffectModuleApi(ch, data.AccessedObject, "Service") {
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.Service<%s>()("%s", {${0}}){}`, effectIdentifier, className, tagKey)
		} else {
			insertText = fmt.Sprintf(`Service<%s>()("%s", {${0}}){}`, className, tagKey)
		}
		items = append(items, makeCompletionItem(
			fmt.Sprintf("Service<%s>", className),
			insertText, sortText, replacementRange,
		))
	}

	// Tag: Effect.Tag("tagKey")<ClassName, {}>(){}
	if isFullyQualified || typeparser.IsNodeReferenceToEffectModuleApi(ch, data.AccessedObject, "Tag") {
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.Tag("%s")<%s, {${0}}>(){}`, effectIdentifier, tagKey, className)
		} else {
			insertText = fmt.Sprintf(`Tag("%s")<%s, {${0}}>(){}`, tagKey, className)
		}
		items = append(items, makeCompletionItem(
			fmt.Sprintf(`Tag("%s")`, className),
			insertText, sortText, replacementRange,
		))
	}

	return items
}
