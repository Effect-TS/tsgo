package completions_test

import (
	"slices"
	"strings"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/fourslash"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"

	_ "github.com/effect-ts/tsgo/etslshooks"
	_ "github.com/effect-ts/tsgo/etstesthooks"
)

const completionTestTsConfig = `{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service"
      }
    ]
  }
}`

const harnessPackageJSON = `{ "name": "@effect/harness-effect-v4" }`

func completionItemsAt(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return completionItemsIn(t, completionTestTsConfig, "", source, position)
}

func completionItemsIn(t *testing.T, tsconfig string, packageJSON string, source string, position int) []*lsproto.CompletionItem {
	t.Helper()

	if position < 0 || position > len(source) {
		t.Fatalf("invalid completion position %d for source length %d", position, len(source))
	}

	content := "// @Filename: /tsconfig.json\n" + tsconfig + "\n"
	if packageJSON != "" {
		content += "// @Filename: /package.json\n" + packageJSON + "\n"
	}
	content += "// @Filename: /test.ts\n" + source[:position] + "/*1*/" + source[position:]

	f, done := fourslash.NewFourslash(t, nil, content)
	defer done()

	f.GoToMarker(t, "1")
	completions := f.GetCompletions(t, nil)
	if completions == nil {
		return nil
	}
	return completions.Items
}

func filterCompletionItems(items []*lsproto.CompletionItem, keep func(*lsproto.CompletionItem) bool) []*lsproto.CompletionItem {
	if len(items) == 0 {
		return nil
	}
	filtered := make([]*lsproto.CompletionItem, 0, len(items))
	for _, item := range items {
		if keep(item) {
			filtered = append(filtered, item)
		}
	}
	if len(filtered) == 0 {
		return nil
	}
	return filtered
}

func findCompletionItem(items []*lsproto.CompletionItem, label string) *lsproto.CompletionItem {
	for _, item := range items {
		if item.Label == label {
			return item
		}
	}
	return nil
}

func fnFunctionStarItems(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsAt(t, source, position), func(item *lsproto.CompletionItem) bool {
		return strings.HasPrefix(item.Label, `fn("`) || item.Label == "fn(function*(){})" || item.Label == "fnUntraced(function*(){})"
	})
}

func labeledItems(t *testing.T, source string, position int, labels ...string) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsAt(t, source, position), func(item *lsproto.CompletionItem) bool {
		return slices.Contains(labels, item.Label)
	})
}

func runCommentDirectiveTriggerCases(t *testing.T, labels ...string) {
	t.Helper()
	for _, tc := range []struct {
		name    string
		source  string
		trigger bool
	}{
		{"line comment", "// @", true},
		{"block comment", "/* @", true},
		{"jsdoc comment", "/** @", true},
		{"extra whitespace", "//  @  ", true},
		{"second line", "const x = 1\n// @", true},
		{"no at sign", "// some comment without at", false},
		{"at outside comment", "const x = @", false},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			items := labeledItems(t, tc.source, len(tc.source), labels...)
			if !tc.trigger {
				if items != nil {
					t.Errorf("expected no items for %q, got %d", tc.source, len(items))
				}
				return
			}
			if len(items) != len(labels) {
				t.Fatalf("expected %d items for %q, got %d", len(labels), tc.source, len(items))
			}
			at := strings.LastIndex(tc.source, "@")
			wantLine := strings.Count(tc.source[:at], "\n")
			wantCharacter := at - (strings.LastIndex(tc.source[:at], "\n") + 1)
			for i, item := range items {
				if item.Label != labels[i] {
					t.Errorf("item[%d].Label = %q, want %q", i, item.Label, labels[i])
				}
				start := item.TextEdit.TextEdit.Range.Start
				if int(start.Line) != wantLine || int(start.Character) != wantCharacter {
					t.Errorf("item[%d] replacement starts at %d:%d, want %d:%d", i, start.Line, start.Character, wantLine, wantCharacter)
				}
			}
		})
	}
}
