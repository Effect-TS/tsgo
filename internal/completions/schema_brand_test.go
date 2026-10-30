package completions_test

import (
	"strings"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
)

func TestSchemaBrand(t *testing.T) {
	t.Parallel()
	const v3Schema = "// @effect-v3\nimport { Schema } from \"effect\"\n"
	for _, tc := range []struct {
		name   string
		source string
		cursor string
		want   string
	}{
		{name: "v3 variable declaration", source: v3Schema + "const UserId = Schema.", want: `brand("UserId")`},
		{name: "v4 is unsupported", source: "import { Schema } from \"effect\"\nconst UserId = Schema."},
		{name: "outside variable declaration", source: v3Schema + "Schema."},
		{name: "inside import declaration", source: v3Schema, cursor: `import { Schema`},
		{name: "after number literal", source: v3Schema + "const x = 42"},
		{name: "after string literal", source: v3Schema + `const x = "hello"`},
		{name: "empty source", source: ""},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			position := len(tc.source)
			if tc.cursor != "" {
				position = strings.Index(tc.source, tc.cursor) + len(tc.cursor)
			}
			items := filterCompletionItems(completionItemsAt(t, tc.source, position), func(item *lsproto.CompletionItem) bool {
				return strings.HasPrefix(item.Label, `brand("`)
			})
			if tc.want == "" {
				if items != nil {
					t.Errorf("expected no brand completions, got %d items", len(items))
				}
				return
			}
			if len(items) != 1 || items[0].Label != tc.want {
				t.Fatalf("expected single %q completion, got %+v", tc.want, items)
			}
		})
	}
}
