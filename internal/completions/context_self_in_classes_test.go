package completions_test

import (
	"strings"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
)

func contextServiceItems(t *testing.T, source string, position int) []*lsproto.CompletionItem {
	t.Helper()
	return filterCompletionItems(completionItemsIn(t, completionTestTsConfig, harnessPackageJSON, source, position), func(item *lsproto.CompletionItem) bool {
		return strings.HasPrefix(item.Label, `Tag("`) || strings.HasPrefix(item.Label, "Service<")
	})
}

func TestContextSelfInClasses_V4Service(t *testing.T) {
	t.Parallel()

	for _, tc := range []struct {
		name      string
		source    string
		cursor    string
		className string
		access    string
	}{
		{
			name: "namespace import",
			source: `import * as Context from "effect/Context"

export class MyService extends Context.`,
			className: "MyService",
			access:    "Context.",
		},
		{
			name: "direct import",
			source: `import { Service } from "effect/Context"

export class MyService extends Service`,
			className: "MyService",
		},
		{
			name: "middle of file",
			source: `import { Effect, Stream } from "effect"
import * as Context from "effect/Context"

class Foo extends Context.S

Stream.unwrap(Effect.gen(function*() {
	const a = yield* Foo

	return Stream.succeed(a.count)
}))`,
			cursor:    "Context.S",
			className: "Foo",
			access:    "Context.",
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			position := len(tc.source)
			if tc.cursor != "" {
				position = strings.Index(tc.source, tc.cursor) + len(tc.cursor)
			}
			items := contextServiceItems(t, tc.source, position)
			if len(items) != 2 {
				t.Fatalf("expected 2 items, got %d", len(items))
			}

			if want := "Service<" + tc.className + ", {}>"; items[0].Label != want {
				t.Errorf("item[0].Label = %q, want %q", items[0].Label, want)
			}
			if got := items[0].TextEdit.TextEdit.NewText; !strings.HasPrefix(got, tc.access+"Service<"+tc.className+", {${0}}>()(\"") || !strings.HasSuffix(got, `"){}`) {
				t.Errorf("item[0].insertText = %q", got)
			}

			if want := "Service<" + tc.className + ">({ make })"; items[1].Label != want {
				t.Errorf("item[1].Label = %q, want %q", items[1].Label, want)
			}
			if got, want := items[1].TextEdit.TextEdit.NewText, tc.access+"Service<"+tc.className+">()(\"@effect/harness-effect-v4/test/"+tc.className+"\", { make: ${0} }){}"; got != want {
				t.Errorf("item[1].insertText = %q, want %q", got, want)
			}
		})
	}
}

func TestContextSelfInClasses_V4IdentifierKeyPattern(t *testing.T) {
	t.Parallel()

	source := `// @test-config { "keyPatterns": [ { "pattern": "package-identifier", "target": "service" } ] }
import * as Context from "effect/Context"

export class MyService extends Context.`
	items := contextServiceItems(t, source, len(source))
	if len(items) != 2 {
		t.Fatalf("expected 2 items, got %d", len(items))
	}

	for i, item := range items {
		if !strings.Contains(item.TextEdit.TextEdit.NewText, `"@effect/harness-effect-v4/test/MyService"`) {
			t.Errorf("item[%d].insertText = %q, want package identifier key", i, item.TextEdit.TextEdit.NewText)
		}
	}
}
