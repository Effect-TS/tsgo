package completions_test

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/fourslash"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
)

const errorKeyPatternTsConfig = `{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "keyPatterns": [
          { "target": "service", "pattern": "default", "skipLeadingPath": ["src/"] },
          { "target": "error", "pattern": "package-identifier" }
        ]
      }
    ]
  }
}`

func taggedItems(t *testing.T, tsconfig string, source string) []*lsproto.CompletionItem {
	t.Helper()
	content := "// @Filename: /tsconfig.json\n" + tsconfig + "\n" +
		"// @Filename: /package.json\n{ \"name\": \"@effect/harness-effect-v4\" }\n" +
		"// @Filename: /test.ts\n" + source + "/*1*/"

	f, done := fourslash.NewFourslash(t, nil, content)
	defer done()

	f.GoToMarker(t, "1")
	completions := f.GetCompletions(t, nil)
	if completions == nil {
		return nil
	}
	return completions.Items
}

func TestErrorKeyPatterns(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name     string
		tsconfig string
		source   string
		label    string
		newText  string
	}{
		{
			name:     "Data.TaggedError uses error key pattern",
			tsconfig: errorKeyPatternTsConfig,
			source:   "import { Data } from \"effect\"\n\nexport class MyError extends Data.",
			label:    `TaggedError("MyError")`,
			newText:  `Data.TaggedError("@effect/harness-effect-v4/test/MyError")<{${0}}>{}`,
		},
		{
			name:     "Data.TaggedClass ignores error key pattern",
			tsconfig: errorKeyPatternTsConfig,
			source:   "import { Data } from \"effect\"\n\nexport class MyError extends Data.",
			label:    `TaggedClass("MyError")`,
			newText:  `Data.TaggedClass("MyError")<{${0}}>{}`,
		},
		{
			name:     "Data.TaggedError without error key pattern keeps class name",
			tsconfig: completionTestTsConfig,
			source:   "import { Data } from \"effect\"\n\nexport class MyError extends Data.",
			label:    `TaggedError("MyError")`,
			newText:  `Data.TaggedError("MyError")<{${0}}>{}`,
		},
		{
			name:     "Schema.TaggedError v4 uses error key pattern",
			tsconfig: errorKeyPatternTsConfig,
			source:   "import { Schema } from \"effect\"\n\nexport class MyError extends Schema.",
			label:    "TaggedError<MyError>",
			newText:  `Schema.TaggedError<MyError, { readonly brand: unique symbol }>()("@effect/harness-effect-v4/test/MyError", {${0}}){}`,
		},
		{
			name:     "Schema.TaggedError v3 uses error key pattern",
			tsconfig: errorKeyPatternTsConfig,
			source:   "// @effect-v3\nimport { Schema } from \"effect\"\n\nexport class MyError extends Schema.",
			label:    "TaggedError<MyError>",
			newText:  `Schema.TaggedError<MyError>()("@effect/harness-effect-v4/test/MyError", {${0}}){}`,
		},
		{
			name:     "Schema.TaggedClass ignores error key pattern",
			tsconfig: errorKeyPatternTsConfig,
			source:   "import { Schema } from \"effect\"\n\nexport class MyError extends Schema.",
			label:    "TaggedClass<MyError>",
			newText:  `Schema.TaggedClass<MyError, { readonly brand: unique symbol }>()("MyError", {${0}}){}`,
		},
		{
			name:     "Schema.TaggedError without error key pattern keeps class name",
			tsconfig: completionTestTsConfig,
			source:   "import { Schema } from \"effect\"\n\nexport class MyError extends Schema.",
			label:    "TaggedError<MyError>",
			newText:  `Schema.TaggedError<MyError, { readonly brand: unique symbol }>()("MyError", {${0}}){}`,
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			item := findCompletionItem(taggedItems(t, tc.tsconfig, tc.source), tc.label)
			if item == nil {
				t.Fatalf("completion %q not found", tc.label)
			}
			if got := item.TextEdit.TextEdit.NewText; got != tc.newText {
				t.Errorf("newText = %q, want %q", got, tc.newText)
			}
		})
	}
}
