package completions

import (
	"fmt"

	"github.com/effect-ts/tsgo/internal/completion"
	"github.com/effect-ts/tsgo/internal/typeparser"
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

	ch := ctx.Checker
	tp := ctx.TypeParser

	// V3 only
	version := tp.SupportedEffectVersion()
	if version != typeparser.EffectMajorV3 {
		return nil
	}

	effectIdentifier := typeparser.FindModuleIdentifier(ctx.SourceFile, "Effect")
	accessedText := data.AccessedObjectText()
	isFullyQualified := effectIdentifier == accessedText
	className := data.ClassNameText()

	// Compute deterministic tag key
	tagKey := computeServiceTagKey(ctx.Program, tp, ch, ctx.SourceFile, className)

	// Build replacement range from byte offsets
	replacementRange := byteSpanToRange(ctx, data.ReplacementStart, data.ReplacementLength)

	sortText := "11"
	var items []*lsproto.CompletionItem

	// Service: Effect.Service<ClassName>()("tagKey", {}){}
	if isFullyQualified || tp.IsNodeReferenceToEffectModuleApi(data.AccessedObject, "Service") {
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.Service<%s>()("%s", {${0}}){}`, effectIdentifier, className, tagKey)
		} else {
			insertText = fmt.Sprintf(`Service<%s>()("%s", {${0}}){}`, className, tagKey)
		}
		items = append(items, makeExtendsCompletionItem(accessedText,
			fmt.Sprintf("Service<%s>", className),
			insertText, sortText, replacementRange,
		))
	}

	// Tag: Effect.Tag("tagKey")<ClassName, {}>(){}
	if isFullyQualified || ctx.TypeParser.IsNodeReferenceToEffectModuleApi(data.AccessedObject, "Tag") {
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.Tag("%s")<%s, {${0}}>(){}`, effectIdentifier, tagKey, className)
		} else {
			insertText = fmt.Sprintf(`Tag("%s")<%s, {${0}}>(){}`, tagKey, className)
		}
		items = append(items, makeExtendsCompletionItem(accessedText,
			fmt.Sprintf(`Tag("%s")`, className),
			insertText, sortText, replacementRange,
		))
	}

	return items
}
