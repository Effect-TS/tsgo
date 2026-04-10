package completions_test

import (
	"slices"
	"strings"
	"testing"

	"github.com/microsoft/typescript-go/shim/fourslash"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"

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

func completionItemsAt(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()

	if position < 0 || position > len(source) {
		t.Fatalf("invalid completion position %d for source length %d", position, len(source))
	}

	sourceWithMarker := source[:position] + "/*1*/" + source[position:]
	content := "// @Filename: /tsconfig.json\n" + completionTestTsConfig + "\n" +
		"// @Filename: /test.ts\n" + sourceWithMarker

	f, done := fourslash.NewFourslash(t, nil, content)
	defer done()

	f.GoToMarker(t, "1")
	completions := f.GetCompletions(t, nil)
	if completions == nil {
		return nil
	}
	return completions.Items
}

func completionItemsAtWithPackageJSON(t *testing.T, packageJSON string, source string, position int) []*lsproto.CompletionItem {
	t.Helper()

	if position < 0 || position > len(source) {
		t.Fatalf("invalid completion position %d for source length %d", position, len(source))
	}

	sourceWithMarker := source[:position] + "/*1*/" + source[position:]
	content := "// @Filename: /tsconfig.json\n" + completionTestTsConfig + "\n" +
		"// @Filename: /package.json\n" + packageJSON + "\n" +
		"// @Filename: /test.ts\n" + sourceWithMarker

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

func effectJsdocCommentItems(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsAt(t, source, position), func(item *lsproto.CompletionItem) bool {
		return item.Label == "@effect-identifier"
	})
}

func effectCodegensCommentItems(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsAt(t, source, position), func(item *lsproto.CompletionItem) bool {
		return item.Label == "@effect-codegens"
	})
}

func effectDiagnosticsCommentItems(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsAt(t, source, position), func(item *lsproto.CompletionItem) bool {
		return item.Label == "@effect-diagnostics" || item.Label == "@effect-diagnostics-next-line"
	})
}

func durationInputItems(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsAt(t, source, position), func(item *lsproto.CompletionItem) bool {
		return slices.Contains([]string{"nanos", "micros", "millis", "seconds", "minutes", "hours", "days", "weeks"}, item.Label)
	})
}

func effectSelfInClassesItems(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsAt(t, source, position), func(item *lsproto.CompletionItem) bool {
		return strings.HasPrefix(item.Label, "Service<") || strings.HasPrefix(item.Label, `Tag("`)
	})
}

func contextSelfInClassesItems(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsAt(t, source, position), func(item *lsproto.CompletionItem) bool {
		return strings.HasPrefix(item.Label, `Tag("`)
	})
}

func contextSelfInClassesItemsWithPackageJSON(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	const packageJSON = `{
	  "name": "@effect/harness-effect-v4"
	}`
	return filterCompletionItems(completionItemsAtWithPackageJSON(t, packageJSON, source, position), func(item *lsproto.CompletionItem) bool {
		return strings.HasPrefix(item.Label, `Tag("`) || strings.HasPrefix(item.Label, "Service<")
	})
}

func effectSQLModelSelfInClassesItems(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsAt(t, source, position), func(item *lsproto.CompletionItem) bool {
		return strings.HasPrefix(item.Label, "Class<")
	})
}

func rpcMakeClassesItems(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsAt(t, source, position), func(item *lsproto.CompletionItem) bool {
		return strings.HasPrefix(item.Label, `make("`)
	})
}

func schemaBrandItems(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsAt(t, source, position), func(item *lsproto.CompletionItem) bool {
		return strings.HasPrefix(item.Label, `brand("`)
	})
}
