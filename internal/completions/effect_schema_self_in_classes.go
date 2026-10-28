package completions

import (
	"fmt"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/completion"
	"github.com/effect-ts/tsgo/internal/pluginoptions"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
)

// effectSchemaSelfInClasses provides completion items for Schema/Model class constructors
// when the cursor is in the extends clause of a class declaration.
// For example, typing `class Foo extends Schema.|` will suggest `Class<Foo>`, `TaggedClass<Foo>`, etc.
var effectSchemaSelfInClasses = completion.Completion{
	Name:        "effectSchemaSelfInClasses",
	Description: "Provides Schema/Model class constructor completions in extends clauses",
	Run:         runEffectSchemaSelfInClasses,
}

func runEffectSchemaSelfInClasses(ctx *completion.Context) []*lsproto.CompletionItem {
	data := completion.ParseDataForExtendsClassCompletion(ctx.SourceFile, ctx.Position)
	if data == nil {
		return nil
	}

	schemaReference := typeparser.FindModuleReference(ctx.SourceFile, "Schema")
	schemaIdentifier := schemaReference.String()
	accessedText := data.AccessedObjectText()
	isFullyQualified := schemaReference.MatchesNode(data.AccessedObject)
	className := data.ClassNameText()

	tp := ctx.TypeParser

	version := tp.SupportedEffectVersion()
	var resolvedOptions *etscore.ResolvedEffectPluginOptions
	if effectConfig := ctx.Program.Options().Effect; effectConfig != nil {
		resolvedOptions = pluginoptions.ResolveEffectPluginOptionsForSourceFile(
			effectConfig,
			ctx.SourceFile.FileName(),
			ctx.Program.Options().ConfigFilePath,
			tspath.UseCaseSensitiveFileNames(ctx.Program),
		)
	}
	// schemaTypeArgs returns the Self type argument, followed by a brand in v4 when the kind is branded.
	schemaTypeArgs := func(kind string) string {
		if version == typeparser.EffectMajorV4 && resolvedOptions.BrandedSchemaClassesIncludes(kind) {
			return className + ", { readonly brand: unique symbol }"
		}
		return className
	}

	// Build replacement range from byte offsets
	replacementRange := byteSpanToRange(ctx, data.ReplacementStart, data.ReplacementLength)

	sortText := "11"
	var items []*lsproto.CompletionItem

	// Schema.Class (both v3 and v4)
	if isFullyQualified || tp.IsNodeReferenceToEffectSchemaModuleApi(data.AccessedObject, "Class") {
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.Class<%s>("%s")({${0}}){}`, schemaIdentifier, schemaTypeArgs(etscore.BrandedSchemaClassClass), className)
		} else {
			insertText = fmt.Sprintf(`Class<%s>("%s")({${0}}){}`, schemaTypeArgs(etscore.BrandedSchemaClassClass), className)
		}
		items = append(items, makeExtendsCompletionItem(accessedText,
			fmt.Sprintf("Class<%s>", className),
			insertText, sortText, replacementRange,
		))
	}

	// Schema.TaggedError (both v3 and v4)
	if isFullyQualified || tp.IsNodeReferenceToEffectSchemaModuleApi(data.AccessedObject, "TaggedError") {
		errorKey := computeDeterministicKey(ctx.Program, tp, ctx.SourceFile, className, "error")
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.TaggedError<%s>()("%s", {${0}}){}`, schemaIdentifier, schemaTypeArgs(etscore.BrandedSchemaClassTaggedError), errorKey)
		} else {
			insertText = fmt.Sprintf(`TaggedError<%s>()("%s", {${0}}){}`, schemaTypeArgs(etscore.BrandedSchemaClassTaggedError), errorKey)
		}
		items = append(items, makeExtendsCompletionItem(accessedText,
			fmt.Sprintf("TaggedError<%s>", className),
			insertText, sortText, replacementRange,
		))
	}

	// Schema.TaggedClass (both v3 and v4)
	if isFullyQualified || tp.IsNodeReferenceToEffectSchemaModuleApi(data.AccessedObject, "TaggedClass") {
		tagKey := computeDeterministicKey(ctx.Program, tp, ctx.SourceFile, className, "taggedClass")
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.TaggedClass<%s>()("%s", {${0}}){}`, schemaIdentifier, schemaTypeArgs(etscore.BrandedSchemaClassTaggedClass), tagKey)
		} else {
			insertText = fmt.Sprintf(`TaggedClass<%s>()("%s", {${0}}){}`, schemaTypeArgs(etscore.BrandedSchemaClassTaggedClass), tagKey)
		}
		items = append(items, makeExtendsCompletionItem(accessedText,
			fmt.Sprintf("TaggedClass<%s>", className),
			insertText, sortText, replacementRange,
		))
	}

	// Schema.TaggedRequest (v3 only)
	if version == typeparser.EffectMajorV3 {
		if isFullyQualified || tp.IsNodeReferenceToEffectSchemaModuleApi(data.AccessedObject, "TaggedRequest") {
			tagKey := computeDeterministicKey(ctx.Program, tp, ctx.SourceFile, className, "taggedClass")
			var insertText string
			if isFullyQualified {
				insertText = fmt.Sprintf(`%s.TaggedRequest<%s>()("%s", {${0}}){}`, schemaIdentifier, className, tagKey)
			} else {
				insertText = fmt.Sprintf(`TaggedRequest<%s>()("%s", {${0}}){}`, className, tagKey)
			}
			items = append(items, makeExtendsCompletionItem(accessedText,
				fmt.Sprintf("TaggedRequest<%s>", className),
				insertText, sortText, replacementRange,
			))
		}
	}

	// Schema.Error (v4 only)
	if version == typeparser.EffectMajorV4 {
		if isFullyQualified || tp.IsNodeReferenceToEffectSchemaModuleApi(data.AccessedObject, "Error") {
			var insertText string
			if isFullyQualified {
				insertText = fmt.Sprintf(`%s.Error<%s>("%s")({${0}}){}`, schemaIdentifier, schemaTypeArgs(etscore.BrandedSchemaClassError), className)
			} else {
				insertText = fmt.Sprintf(`Error<%s>("%s")({${0}}){}`, schemaTypeArgs(etscore.BrandedSchemaClassError), className)
			}
			items = append(items, makeExtendsCompletionItem(accessedText,
				fmt.Sprintf("Error<%s>", className),
				insertText, sortText, replacementRange,
			))
		}
		if isFullyQualified || tp.IsNodeReferenceToEffectSchemaModuleApi(data.AccessedObject, "Opaque") {
			var insertText string
			if isFullyQualified {
				insertText = fmt.Sprintf(`%s.Opaque<%s>()(${0}){}`, schemaIdentifier, schemaTypeArgs(etscore.BrandedSchemaClassOpaque))
			} else {
				insertText = fmt.Sprintf(`Opaque<%s>()(${0}){}`, schemaTypeArgs(etscore.BrandedSchemaClassOpaque))
			}
			items = append(items, makeExtendsCompletionItem(accessedText,
				fmt.Sprintf("Opaque<%s>", className),
				insertText, sortText, replacementRange,
			))
		}
	}

	// Model.Class (v4 only)
	if version == typeparser.EffectMajorV4 {
		modelReference := typeparser.FindModuleReferenceForPackage(ctx.SourceFile, "effect/schema", "Model")
		modelIdentifier := modelReference.String()

		isModelFullyQualified := modelReference.MatchesNode(data.AccessedObject)

		if isModelFullyQualified || tp.IsNodeReferenceToEffectModelModuleApi(data.AccessedObject, "Class") {
			var insertText string
			if isModelFullyQualified {
				insertText = fmt.Sprintf(`%s.Class<%s>("%s")({${0}}){}`, modelIdentifier, className, className)
			} else {
				insertText = fmt.Sprintf(`Class<%s>("%s")({${0}}){}`, className, className)
			}
			items = append(items, makeExtendsCompletionItem(accessedText,
				fmt.Sprintf("Class<%s>", className),
				insertText, sortText, replacementRange,
			))
		}
	}

	return items
}

// makeCompletionItem creates a CompletionItem with snippet format and a text edit.
func makeCompletionItem(label string, insertText string, sortText string, replacementRange lsproto.Range, kindOverride ...lsproto.CompletionItemKind) *lsproto.CompletionItem {
	kind := lsproto.CompletionItemKindVariable
	if len(kindOverride) > 0 {
		kind = kindOverride[0]
	}
	format := lsproto.InsertTextFormatSnippet
	return &lsproto.CompletionItem{
		Label:            label,
		Kind:             &kind,
		InsertTextFormat: &format,
		SortText:         &sortText,
		TextEdit: &lsproto.TextEditOrInsertReplaceEdit{
			TextEdit: &lsproto.TextEdit{
				NewText: insertText,
				Range:   replacementRange,
			},
		},
	}
}

// makeExtendsCompletionItem creates a CompletionItem for an extends-clause completion.
// It sets FilterText to accessedText.label so VS Code can filter correctly when the
// replacement range covers the full "Module.Property" expression.
func makeExtendsCompletionItem(accessedText string, label string, insertText string, sortText string, replacementRange lsproto.Range) *lsproto.CompletionItem {
	item := makeCompletionItem(label, insertText, sortText, replacementRange)
	filterText := accessedText + "." + label
	item.FilterText = &filterText
	return item
}

// byteSpanToRange converts a byte offset span to an LSP Range.
func byteSpanToRange(ctx *completion.Context, start int, length int) lsproto.Range {
	startLine, startChar := scanner.GetECMALineAndUTF16CharacterOfPosition(ctx.SourceFile, start)
	endLine, endChar := scanner.GetECMALineAndUTF16CharacterOfPosition(ctx.SourceFile, start+length)
	return lsproto.Range{
		Start: lsproto.Position{Line: uint32(startLine), Character: uint32(startChar)},
		End:   lsproto.Position{Line: uint32(endLine), Character: uint32(endChar)},
	}
}
