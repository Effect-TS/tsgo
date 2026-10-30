package completions_test

import "testing"

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

const taggedClassKeyPatternTsConfig = `{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "plugins": [
      {
        "name": "@effect/language-service",
        "keyPatterns": [
          { "target": "taggedClass", "pattern": "package-identifier" }
        ]
      }
    ]
  }
}`

func TestErrorKeyPatterns(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name        string
		tsconfig    string
		packageJson string
		source      string
		label       string
		newText     string
	}{
		{
			name:     "Data.TaggedError uses error key pattern",
			tsconfig: errorKeyPatternTsConfig,
			source:   "import { Data } from \"effect\"\n\nexport class MyError extends Data.",
			label:    `TaggedError("MyError")`,
			newText:  `Data.TaggedError("@effect/harness-effect-v4/test/MyError")<{${0}}>{}`,
		},
		{
			name:        "Data.TaggedError without package name omits package from key",
			tsconfig:    errorKeyPatternTsConfig,
			packageJson: `{ "version": "1.0.0" }`,
			source:      "import { Data } from \"effect\"\n\nexport class MyError extends Data.",
			label:       `TaggedError("MyError")`,
			newText:     `Data.TaggedError("test/MyError")<{${0}}>{}`,
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
			name:     "Data.TaggedClass uses taggedClass key pattern",
			tsconfig: taggedClassKeyPatternTsConfig,
			source:   "import { Data } from \"effect\"\n\nexport class Completed extends Data.",
			label:    `TaggedClass("Completed")`,
			newText:  `Data.TaggedClass("@effect/harness-effect-v4/test/Completed")<{${0}}>{}`,
		},
		{
			name:     "Data.TaggedError ignores taggedClass key pattern",
			tsconfig: taggedClassKeyPatternTsConfig,
			source:   "import { Data } from \"effect\"\n\nexport class Completed extends Data.",
			label:    `TaggedError("Completed")`,
			newText:  `Data.TaggedError("Completed")<{${0}}>{}`,
		},
		{
			name:     "Schema.TaggedClass v4 uses taggedClass key pattern",
			tsconfig: taggedClassKeyPatternTsConfig,
			source:   "import { Schema } from \"effect\"\n\nexport class Completed extends Schema.",
			label:    "TaggedClass<Completed>",
			newText:  `Schema.TaggedClass<Completed, { readonly brand: unique symbol }>()("@effect/harness-effect-v4/test/Completed", {${0}}){}`,
		},
		{
			name:     "Schema.TaggedClass v3 uses taggedClass key pattern",
			tsconfig: taggedClassKeyPatternTsConfig,
			source:   "// @effect-v3\nimport { Schema } from \"effect\"\n\nexport class Completed extends Schema.",
			label:    "TaggedClass<Completed>",
			newText:  `Schema.TaggedClass<Completed>()("@effect/harness-effect-v4/test/Completed", {${0}}){}`,
		},
		{
			name:     "Schema.TaggedRequest v3 uses taggedClass key pattern",
			tsconfig: taggedClassKeyPatternTsConfig,
			source:   "// @effect-v3\nimport { Schema } from \"effect\"\n\nexport class Completed extends Schema.",
			label:    "TaggedRequest<Completed>",
			newText:  `Schema.TaggedRequest<Completed>()("@effect/harness-effect-v4/test/Completed", {${0}}){}`,
		},
		{
			name:     "Schema.TaggedClass without taggedClass key pattern keeps class name",
			tsconfig: completionTestTsConfig,
			source:   "import { Schema } from \"effect\"\n\nexport class Completed extends Schema.",
			label:    "TaggedClass<Completed>",
			newText:  `Schema.TaggedClass<Completed, { readonly brand: unique symbol }>()("Completed", {${0}}){}`,
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
			packageJson := tc.packageJson
			if packageJson == "" {
				packageJson = harnessPackageJSON
			}
			item := findCompletionItem(completionItemsIn(t, tc.tsconfig, packageJson, tc.source, len(tc.source)), tc.label)
			if item == nil {
				t.Fatalf("completion %q not found", tc.label)
			}
			if got := item.TextEdit.TextEdit.NewText; got != tc.newText {
				t.Errorf("newText = %q, want %q", got, tc.newText)
			}
		})
	}
}
