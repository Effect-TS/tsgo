package effecttest_test

import (
	"strings"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/fourslash"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"

	_ "github.com/effect-ts/tsgo/etslshooks"
	_ "github.com/effect-ts/tsgo/etstesthooks"
)

func completionItemsAtMarker(t *testing.T, content string) []*lsproto.CompletionItem {
	t.Helper()

	f, done := fourslash.NewFourslash(t, nil, content)
	defer done()

	f.GoToMarker(t, "1")
	completions := f.GetCompletions(t, nil)
	if completions == nil {
		t.Fatal("completions is nil")
	}
	return completions.Items
}

func findCompletionItem(items []*lsproto.CompletionItem, label string) *lsproto.CompletionItem {
	for _, item := range items {
		if item.Label == label {
			return item
		}
	}
	return nil
}

func TestEffectCompletionExtendsBarrelNamespace(t *testing.T) {
	t.Parallel()

	items := completionItemsAtMarker(t, pluginTestContent("", `import * as X from "effect"
class MyService extends X.Context./*1*/`))
	item := findCompletionItem(items, "Service<MyService, {}>")
	if item == nil {
		t.Fatal("expected 'Service<MyService, {}>' completion")
	}
	if item.TextEdit == nil || item.TextEdit.TextEdit == nil || !strings.HasPrefix(item.TextEdit.TextEdit.NewText, "X.Context.Service<MyService, {") {
		t.Errorf("expected the completion to reference the module through the barrel namespace, got %+v", item.TextEdit)
	}
}

func TestEffectCompletionDisabled(t *testing.T) {
	t.Parallel()

	items := completionItemsAtMarker(t, pluginTestContent(`,
        "completions": false`, `import * as Context from "effect/Context"
class MyService extends Context./*1*/`))
	for _, label := range []string{"Service<MyService, {}>", "Service<MyService>({ make })"} {
		if findCompletionItem(items, label) != nil {
			t.Errorf("did not expect Effect completion %q when completions=false", label)
		}
	}
}
