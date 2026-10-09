package completions_test

import (
	"strings"
	"testing"

	"github.com/google/go-cmp/cmp"
	"github.com/microsoft/TypeScript/tsc/shim/fourslash"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"

	_ "github.com/effect-ts/tsgo/etscheckerhooks"
)

var schemaClassCompletionCases = []struct {
	version   string
	directive string
	snippets  []struct {
		api     string
		newText string
	}
}{
	{
		version: "v4",
		snippets: []struct {
			api     string
			newText string
		}{
			{"Class", `Class<Foo, { readonly brand: unique symbol }>("Foo")({${0}}){}`},
			{"Error", `Error<Foo, { readonly brand: unique symbol }>("Foo")({${0}}){}`},
			{"Opaque", `Opaque<Foo, { readonly brand: unique symbol }>()(${0}){}`},
			{"TaggedClass", `TaggedClass<Foo, { readonly brand: unique symbol }>()("Foo", {${0}}){}`},
			{"TaggedError", `TaggedError<Foo, { readonly brand: unique symbol }>()("Foo", {${0}}){}`},
		},
	},
	{
		version:   "v3",
		directive: "// @effect-v3\n",
		snippets: []struct {
			api     string
			newText string
		}{
			{"Class", `Class<Foo>("Foo")({${0}}){}`},
			{"TaggedClass", `TaggedClass<Foo>()("Foo", {${0}}){}`},
			{"TaggedError", `TaggedError<Foo>()("Foo", {${0}}){}`},
			{"TaggedRequest", `TaggedRequest<Foo>()("Foo", {${0}}){}`},
		},
	},
}

func schemaClassCompletionItems(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsAt(t, source, position), func(item *lsproto.CompletionItem) bool {
		return strings.HasSuffix(item.Label, "<Foo>")
	})
}

func TestEffectSchemaSelfInClasses_ModuleImports(t *testing.T) {
	t.Parallel()
	for _, version := range schemaClassCompletionCases {
		for _, module := range []struct {
			name       string
			importText string
			identifier string
			member     string
		}{
			{"barrel", `import { Schema } from "effect"`, "Schema", ""},
			{"namespace", `import * as Schema from "effect/Schema"`, "Schema", ""},
			{"barrel alias", `import { Schema as S } from "effect"`, "S", ""},
			{"namespace alias", `import * as S from "effect/Schema"`, "S", ""},
			{"partial member", `import { Schema } from "effect"`, "Schema", "Tagg"},
		} {
			t.Run(version.version+"/"+module.name, func(t *testing.T) {
				t.Parallel()
				source := version.directive + module.importText + "\nclass Foo extends " + module.identifier + "." + module.member
				items := schemaClassCompletionItems(t, source, len(source))
				if len(items) != len(version.snippets) {
					t.Errorf("got %d custom items, want %d", len(items), len(version.snippets))
				}
				for i, expected := range version.snippets {
					label := expected.api + "<Foo>"
					item := findCompletionItem(items, label)
					if item == nil || item.TextEdit == nil || item.TextEdit.TextEdit == nil {
						t.Errorf("missing snippet text edit for %q", label)
						continue
					}
					if got, want := item.TextEdit.TextEdit.NewText, module.identifier+"."+expected.newText; got != want {
						t.Errorf("%s NewText = %q, want %q", label, got, want)
					}
					if i < len(items) && items[i].Label != label {
						t.Errorf("item[%d].Label = %q, want %q", i, items[i].Label, label)
					}
				}
			})
		}
	}
}

func TestEffectSchemaSelfInClasses_DirectImports(t *testing.T) {
	t.Parallel()
	for _, version := range schemaClassCompletionCases {
		for _, expected := range version.snippets {
			t.Run(version.version+"/"+expected.api, func(t *testing.T) {
				t.Parallel()
				source := version.directive + `import { ` + expected.api + ` } from "effect/Schema"` + "\nclass Foo extends " + expected.api
				items := schemaClassCompletionItems(t, source, len(source))
				if len(items) != 1 {
					t.Fatalf("got %d custom items, want 1", len(items))
				}
				item := items[0]
				if item.Label != expected.api+"<Foo>" || item.TextEdit == nil || item.TextEdit.TextEdit == nil {
					t.Fatalf("expected %s<Foo> snippet text edit, got %+v", expected.api, item)
				}
				if got := item.TextEdit.TextEdit.NewText; got != expected.newText {
					t.Errorf("NewText = %q, want %q", got, expected.newText)
				}
			})
		}
	}
}

func TestEffectSchemaSelfInClasses_MetadataAndRange(t *testing.T) {
	t.Parallel()
	source := `import { Schema } from "effect"
class Foo extends Schema.Tagg
const after = 1`
	position := strings.Index(source, "Schema.Tagg") + len("Schema.Tagg")
	items := schemaClassCompletionItems(t, source, position)
	item := findCompletionItem(items, "TaggedClass<Foo>")
	kind := lsproto.CompletionItemKindVariable
	format := lsproto.InsertTextFormatSnippet
	sortText := "11"
	filterText := "Schema.TaggedClass<Foo>"
	expected := &lsproto.CompletionItem{
		Label:            "TaggedClass<Foo>",
		Kind:             &kind,
		InsertTextFormat: &format,
		SortText:         &sortText,
		FilterText:       &filterText,
		TextEdit: &lsproto.TextEditOrInsertReplaceEdit{
			TextEdit: &lsproto.TextEdit{
				NewText: `Schema.TaggedClass<Foo, { readonly brand: unique symbol }>()("Foo", {${0}}){}`,
				Range: lsproto.Range{
					Start: lsproto.Position{Line: 1, Character: 18},
					End:   lsproto.Position{Line: 1, Character: 29},
				},
			},
		},
		Data: &lsproto.CompletionItemData{
			FileName: "/test.ts",
			Position: 61,
			Name:     "TaggedClass<Foo>",
		},
	}
	if diff := cmp.Diff(expected, item); diff != "" {
		t.Errorf("completion mismatch (-want +got):\n%s", diff)
	}
}

func TestEffectSchemaSelfInClasses_ModelUnchanged(t *testing.T) {
	t.Parallel()
	for _, test := range []struct {
		source  string
		newText string
	}{
		{"import { Model } from \"effect/schema\"\nclass Foo extends Model.", `Model.Class<Foo>("Foo")({${0}}){}`},
		{"import * as Model from \"effect/schema/Model\"\nclass Foo extends Model.", `Model.Class<Foo>("Foo")({${0}}){}`},
		{"import { Class } from \"effect/schema/Model\"\nclass Foo extends Class", `Class<Foo>("Foo")({${0}}){}`},
	} {
		items := schemaClassCompletionItems(t, test.source, len(test.source))
		if len(items) != 1 || items[0].TextEdit == nil || items[0].TextEdit.TextEdit == nil {
			t.Fatalf("expected one Model snippet text edit, got %+v", items)
		}
		if got := items[0].TextEdit.TextEdit.NewText; got != test.newText {
			t.Errorf("NewText = %q, want %q", got, test.newText)
		}
	}
}

func TestEffectSchemaSelfInClasses_OutsideSchemaExtends(t *testing.T) {
	t.Parallel()
	positive := "import { Schema } from \"effect\"\nclass Foo extends Schema."
	if items := schemaClassCompletionItems(t, positive, len(positive)); len(items) != 5 {
		t.Fatalf("got %d Schema extends snippets, want 5", len(items))
	}
	for _, source := range []string{
		"import { Schema } from \"effect\"\nconst value = Schema.",
		"import { Schema } from \"effect\"\ninterface Foo extends Schema.Class",
		`import { Schema } from "effect"
namespace Local {
  export declare function Class(): void
  export declare function TaggedClass(): void
  export declare function TaggedError(): void
  export declare function Error(): void
  export declare function Opaque(): void
}
class Foo extends Local.`,
	} {
		if items := schemaClassCompletionItems(t, source, len(source)); len(items) != 0 {
			t.Errorf("unexpected Schema extends snippets for %q: %+v", source, items)
		}
	}
}

func TestEffectSchemaSelfInClasses_AcceptedSnippet(t *testing.T) {
	t.Parallel()
	for _, expected := range schemaClassCompletionCases[0].snippets {
		t.Run(expected.api, func(t *testing.T) {
			t.Parallel()
			config := strings.Replace(completionTestTsConfig, `"name": "@effect/language-service"`, `"name": "@effect/language-service", "diagnosticSeverity": { "schemaClassMissingBrand": "error" }`, 1)
			const source = "import { Schema } from \"effect\"\nclass Foo extends Schema."
			content := "// @Filename: /tsconfig.json\n" + config + "\n// @Filename: /test.ts\n" + source + "/*1*/"
			f, done := fourslash.NewFourslash(t, nil, content)
			defer done()
			f.GoToMarker(t, "1")
			completions := f.GetCompletions(t, nil)
			if completions == nil {
				t.Fatal("missing completions")
			}
			item := findCompletionItem(completions.Items, expected.api+"<Foo>")
			if item == nil || item.TextEdit == nil || item.TextEdit.TextEdit == nil {
				t.Fatal("missing snippet text edit")
			}
			placeholder := ""
			if expected.api == "Opaque" {
				placeholder = "Schema.Struct({})"
			}
			snippet := strings.ReplaceAll(item.TextEdit.TextEdit.NewText, "${0}", placeholder)
			editRange := item.TextEdit.TextEdit.Range
			lineStart := strings.Index(source, "\n") + 1
			start := lineStart + int(editRange.Start.Character)
			length := int(editRange.End.Character - editRange.Start.Character)
			f.Replace(t, start, length, snippet)
			f.VerifyNoErrors(t)
			unbranded := strings.Replace(snippet, ", { readonly brand: unique symbol }", "", 1)
			f.Replace(t, start, len(snippet), unbranded)
			f.VerifyNumberOfErrorsInCurrentFile(t, 1)
		})
	}
}
