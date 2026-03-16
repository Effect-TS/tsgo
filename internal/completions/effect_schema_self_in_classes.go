package completions

import (
	"fmt"

	"github.com/effect-ts/effect-typescript-go/internal/completion"
	"github.com/effect-ts/effect-typescript-go/internal/effectutil"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/scanner"
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

	schemaIdentifier := effectutil.FindModuleIdentifier(ctx.SourceFile, "Schema")
	accessedText := data.AccessedObjectText()
	isFullyQualified := schemaIdentifier == accessedText
	className := data.ClassNameText()

	// Get checker for version detection and API reference checks
	ch, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	defer done()

	version := typeparser.SupportedEffectVersion(ch)

	// Build replacement range from byte offsets
	replacementRange := byteSpanToRange(ctx, data.ReplacementStart, data.ReplacementLength)

	sortText := "11"
	var items []*lsproto.CompletionItem

	// Schema.Class (both v3 and v4)
	if isFullyQualified || typeparser.IsNodeReferenceToEffectSchemaModuleApi(ch, data.AccessedObject, "Class") {
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.Class<%s>("%s")({${0}}){}`, schemaIdentifier, className, className)
		} else {
			insertText = fmt.Sprintf(`Class<%s>("%s")({${0}}){}`, className, className)
		}
		items = append(items, makeExtendsCompletionItem(accessedText,
			fmt.Sprintf("Class<%s>", className),
			insertText, sortText, replacementRange,
		))
	}

	// Schema.TaggedError (v3 only)
	if version == typeparser.EffectMajorV3 {
		if isFullyQualified || typeparser.IsNodeReferenceToEffectSchemaModuleApi(ch, data.AccessedObject, "TaggedError") {
			var insertText string
			if isFullyQualified {
				insertText = fmt.Sprintf(`%s.TaggedError<%s>()("%s", {${0}}){}`, schemaIdentifier, className, className)
			} else {
				insertText = fmt.Sprintf(`TaggedError<%s>()("%s", {${0}}){}`, className, className)
			}
			items = append(items, makeExtendsCompletionItem(accessedText,
				fmt.Sprintf("TaggedError<%s>", className),
				insertText, sortText, replacementRange,
			))
		}
	}

	// Schema.TaggedErrorClass (v4 only)
	if version == typeparser.EffectMajorV4 {
		if isFullyQualified || typeparser.IsNodeReferenceToEffectSchemaModuleApi(ch, data.AccessedObject, "TaggedErrorClass") {
			var insertText string
			if isFullyQualified {
				insertText = fmt.Sprintf(`%s.TaggedErrorClass<%s>()("%s", {${0}}){}`, schemaIdentifier, className, className)
			} else {
				insertText = fmt.Sprintf(`TaggedErrorClass<%s>()("%s", {${0}}){}`, className, className)
			}
			items = append(items, makeExtendsCompletionItem(accessedText,
				fmt.Sprintf("TaggedErrorClass<%s>", className),
				insertText, sortText, replacementRange,
			))
		}
	}

	// Schema.TaggedClass (both v3 and v4)
	if isFullyQualified || typeparser.IsNodeReferenceToEffectSchemaModuleApi(ch, data.AccessedObject, "TaggedClass") {
		var insertText string
		if isFullyQualified {
			insertText = fmt.Sprintf(`%s.TaggedClass<%s>()("%s", {${0}}){}`, schemaIdentifier, className, className)
		} else {
			insertText = fmt.Sprintf(`TaggedClass<%s>()("%s", {${0}}){}`, className, className)
		}
		items = append(items, makeExtendsCompletionItem(accessedText,
			fmt.Sprintf("TaggedClass<%s>", className),
			insertText, sortText, replacementRange,
		))
	}

	// Schema.TaggedRequest (v3 only)
	if version == typeparser.EffectMajorV3 {
		if isFullyQualified || typeparser.IsNodeReferenceToEffectSchemaModuleApi(ch, data.AccessedObject, "TaggedRequest") {
			var insertText string
			if isFullyQualified {
				insertText = fmt.Sprintf(`%s.TaggedRequest<%s>()("%s", {${0}}){}`, schemaIdentifier, className, className)
			} else {
				insertText = fmt.Sprintf(`TaggedRequest<%s>()("%s", {${0}}){}`, className, className)
			}
			items = append(items, makeExtendsCompletionItem(accessedText,
				fmt.Sprintf("TaggedRequest<%s>", className),
				insertText, sortText, replacementRange,
			))
		}
	}

	// Schema.ErrorClass (v4 only)
	if version == typeparser.EffectMajorV4 {
		if isFullyQualified || typeparser.IsNodeReferenceToEffectSchemaModuleApi(ch, data.AccessedObject, "ErrorClass") {
			var insertText string
			if isFullyQualified {
				insertText = fmt.Sprintf(`%s.ErrorClass<%s>("%s")({${0}}){}`, schemaIdentifier, className, className)
			} else {
				insertText = fmt.Sprintf(`ErrorClass<%s>("%s")({${0}}){}`, className, className)
			}
			items = append(items, makeExtendsCompletionItem(accessedText,
				fmt.Sprintf("ErrorClass<%s>", className),
				insertText, sortText, replacementRange,
			))
		}
	}

	// Model.Class (v4 only)
	if version == typeparser.EffectMajorV4 {
		modelIdentifier := effectutil.FindModuleIdentifierForPackage(ctx.SourceFile, "effect/unstable", "schema")
		if modelIdentifier == "schema" {
			// Fallback: try effect/unstable barrel
			modelIdentifier = effectutil.FindModuleIdentifierForPackage(ctx.SourceFile, "effect/unstable", "Model")
		}

		isModelFullyQualified := modelIdentifier == accessedText

		if isModelFullyQualified || typeparser.IsNodeReferenceToEffectModelModuleApi(ch, data.AccessedObject, "Class") {
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
		Label:           label,
		Kind:            &kind,
		InsertTextFormat: &format,
		SortText:        &sortText,
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
