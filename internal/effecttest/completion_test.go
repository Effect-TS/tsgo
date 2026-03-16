package effecttest_test

import (
	"testing"

	"github.com/microsoft/typescript-go/shim/fourslash"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"

	_ "github.com/effect-ts/effect-typescript-go/etslshooks"
	_ "github.com/effect-ts/effect-typescript-go/etstesthooks"
)

func findCompletionLabel(items []*lsproto.CompletionItem, label string) bool {
	for _, item := range items {
		if item.Label == label {
			return true
		}
	}
	return false
}

func TestEffectCompletionExtendsServiceMap(t *testing.T) {
	t.Parallel()

	const content = `// @Filename: /tsconfig.json
{
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
}
// @Filename: /test.ts
import { ServiceMap } from "effect"
class MyService extends ServiceMap./*1*/`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	f.GoToMarker(t, "1")
	completions := f.GetCompletions(t, nil)
	if completions == nil {
		t.Fatal("completions is nil")
	}

	if !findCompletionLabel(completions.Items, "Service<MyService, {}>") {
		t.Error("expected 'Service<MyService, {}>' completion")
	}
	if !findCompletionLabel(completions.Items, "Service<MyService>({ make })") {
		t.Error("expected 'Service<MyService>({ make })' completion")
	}
}

func TestEffectCompletionExtendsSchema(t *testing.T) {
	t.Parallel()

	const content = `// @Filename: /tsconfig.json
{
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
}
// @Filename: /test.ts
import { Schema } from "effect"
class Foo extends Schema./*1*/`

	f, done := fourslash.NewFourslash(t, nil /*capabilities*/, content)
	defer done()

	f.GoToMarker(t, "1")
	completions := f.GetCompletions(t, nil)
	if completions == nil {
		t.Fatal("completions is nil")
	}

	if !findCompletionLabel(completions.Items, "Class<Foo>") {
		t.Error("expected 'Class<Foo>' completion")
	}
	if !findCompletionLabel(completions.Items, "TaggedClass<Foo>") {
		t.Error("expected 'TaggedClass<Foo>' completion")
	}
}
