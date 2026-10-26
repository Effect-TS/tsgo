use ets_go::testing::T;
use ets_shim::lsproto;

use crate::*;

// go test: internal/completions/error_key_patterns_test.go:10 errorKeyPatternTsConfig
const ERROR_KEY_PATTERN_TS_CONFIG: &str = r#"{
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
}"#;

// go test: internal/completions/error_key_patterns_test.go:28 taggedClassKeyPatternTsConfig
const TAGGED_CLASS_KEY_PATTERN_TS_CONFIG: &str = r#"{
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
}"#;

// go test: internal/completions/error_key_patterns_test.go:45 namedPackageJson
const NAMED_PACKAGE_JSON: &str = r#"{ "name": "@effect/harness-effect-v4" }"#;

// go test: internal/completions/error_key_patterns_test.go:47 taggedItems
fn tagged_items(
    t: &T,
    tsconfig: &str,
    package_json: &str,
    source: &str,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    let content = format!(
        "// @Filename: /tsconfig.json\n{tsconfig}\n// @Filename: /package.json\n{package_json}\n// @Filename: /test.ts\n{source}/*1*/"
    );

    let mut f = ets_fourslash::new_fourslash(t, None, &content);

    f.go_to_marker(t, "1");
    let completions = f.get_completions(t, None);
    f.done(t);
    let Some(completions) = completions else {
        return Vec::new();
    };
    completions.items
}

// go test: internal/completions/error_key_patterns_test.go:64 TestErrorKeyPatterns
#[test]
fn test_error_key_patterns() {
    go_test(|t| {
        t.parallel();

        struct Case {
            name: &'static str,
            tsconfig: &'static str,
            package_json: &'static str,
            source: &'static str,
            label: &'static str,
            new_text: &'static str,
        }
        let cases = [
            Case {
                name: "Data.TaggedError uses error key pattern",
                tsconfig: ERROR_KEY_PATTERN_TS_CONFIG,
                package_json: "",
                source: "import { Data } from \"effect\"\n\nexport class MyError extends Data.",
                label: r#"TaggedError("MyError")"#,
                new_text: r#"Data.TaggedError("@effect/harness-effect-v4/test/MyError")<{${0}}>{}"#,
            },
            Case {
                name: "Data.TaggedError without package name omits package from key",
                tsconfig: ERROR_KEY_PATTERN_TS_CONFIG,
                package_json: r#"{ "version": "1.0.0" }"#,
                source: "import { Data } from \"effect\"\n\nexport class MyError extends Data.",
                label: r#"TaggedError("MyError")"#,
                new_text: r#"Data.TaggedError("test/MyError")<{${0}}>{}"#,
            },
            Case {
                name: "Data.TaggedClass ignores error key pattern",
                tsconfig: ERROR_KEY_PATTERN_TS_CONFIG,
                package_json: "",
                source: "import { Data } from \"effect\"\n\nexport class MyError extends Data.",
                label: r#"TaggedClass("MyError")"#,
                new_text: r#"Data.TaggedClass("MyError")<{${0}}>{}"#,
            },
            Case {
                name: "Data.TaggedError without error key pattern keeps class name",
                tsconfig: COMPLETION_TEST_TS_CONFIG,
                package_json: "",
                source: "import { Data } from \"effect\"\n\nexport class MyError extends Data.",
                label: r#"TaggedError("MyError")"#,
                new_text: r#"Data.TaggedError("MyError")<{${0}}>{}"#,
            },
            Case {
                name: "Schema.TaggedError v4 uses error key pattern",
                tsconfig: ERROR_KEY_PATTERN_TS_CONFIG,
                package_json: "",
                source: "import { Schema } from \"effect\"\n\nexport class MyError extends Schema.",
                label: "TaggedError<MyError>",
                new_text: r#"Schema.TaggedError<MyError, { readonly brand: unique symbol }>()("@effect/harness-effect-v4/test/MyError", {${0}}){}"#,
            },
            Case {
                name: "Schema.TaggedError v3 uses error key pattern",
                tsconfig: ERROR_KEY_PATTERN_TS_CONFIG,
                package_json: "",
                source: "// @effect-v3\nimport { Schema } from \"effect\"\n\nexport class MyError extends Schema.",
                label: "TaggedError<MyError>",
                new_text: r#"Schema.TaggedError<MyError>()("@effect/harness-effect-v4/test/MyError", {${0}}){}"#,
            },
            Case {
                name: "Schema.TaggedClass ignores error key pattern",
                tsconfig: ERROR_KEY_PATTERN_TS_CONFIG,
                package_json: "",
                source: "import { Schema } from \"effect\"\n\nexport class MyError extends Schema.",
                label: "TaggedClass<MyError>",
                new_text: r#"Schema.TaggedClass<MyError, { readonly brand: unique symbol }>()("MyError", {${0}}){}"#,
            },
            Case {
                name: "Data.TaggedClass uses taggedClass key pattern",
                tsconfig: TAGGED_CLASS_KEY_PATTERN_TS_CONFIG,
                package_json: "",
                source: "import { Data } from \"effect\"\n\nexport class Completed extends Data.",
                label: r#"TaggedClass("Completed")"#,
                new_text: r#"Data.TaggedClass("@effect/harness-effect-v4/test/Completed")<{${0}}>{}"#,
            },
            Case {
                name: "Data.TaggedError ignores taggedClass key pattern",
                tsconfig: TAGGED_CLASS_KEY_PATTERN_TS_CONFIG,
                package_json: "",
                source: "import { Data } from \"effect\"\n\nexport class Completed extends Data.",
                label: r#"TaggedError("Completed")"#,
                new_text: r#"Data.TaggedError("Completed")<{${0}}>{}"#,
            },
            Case {
                name: "Schema.TaggedClass v4 uses taggedClass key pattern",
                tsconfig: TAGGED_CLASS_KEY_PATTERN_TS_CONFIG,
                package_json: "",
                source: "import { Schema } from \"effect\"\n\nexport class Completed extends Schema.",
                label: "TaggedClass<Completed>",
                new_text: r#"Schema.TaggedClass<Completed, { readonly brand: unique symbol }>()("@effect/harness-effect-v4/test/Completed", {${0}}){}"#,
            },
            Case {
                name: "Schema.TaggedClass v3 uses taggedClass key pattern",
                tsconfig: TAGGED_CLASS_KEY_PATTERN_TS_CONFIG,
                package_json: "",
                source: "// @effect-v3\nimport { Schema } from \"effect\"\n\nexport class Completed extends Schema.",
                label: "TaggedClass<Completed>",
                new_text: r#"Schema.TaggedClass<Completed>()("@effect/harness-effect-v4/test/Completed", {${0}}){}"#,
            },
            Case {
                name: "Schema.TaggedRequest v3 uses taggedClass key pattern",
                tsconfig: TAGGED_CLASS_KEY_PATTERN_TS_CONFIG,
                package_json: "",
                source: "// @effect-v3\nimport { Schema } from \"effect\"\n\nexport class Completed extends Schema.",
                label: "TaggedRequest<Completed>",
                new_text: r#"Schema.TaggedRequest<Completed>()("@effect/harness-effect-v4/test/Completed", {${0}}){}"#,
            },
            Case {
                name: "Schema.TaggedClass without taggedClass key pattern keeps class name",
                tsconfig: COMPLETION_TEST_TS_CONFIG,
                package_json: "",
                source: "import { Schema } from \"effect\"\n\nexport class Completed extends Schema.",
                label: "TaggedClass<Completed>",
                new_text: r#"Schema.TaggedClass<Completed, { readonly brand: unique symbol }>()("Completed", {${0}}){}"#,
            },
            Case {
                name: "Schema.TaggedError without error key pattern keeps class name",
                tsconfig: COMPLETION_TEST_TS_CONFIG,
                package_json: "",
                source: "import { Schema } from \"effect\"\n\nexport class MyError extends Schema.",
                label: "TaggedError<MyError>",
                new_text: r#"Schema.TaggedError<MyError, { readonly brand: unique symbol }>()("MyError", {${0}}){}"#,
            },
        ];

        for tc in &cases {
            t.run(tc.name, |t| {
                t.parallel();
                let mut package_json = tc.package_json;
                if package_json.is_empty() {
                    package_json = NAMED_PACKAGE_JSON;
                }
                let items = tagged_items(t, tc.tsconfig, package_json, tc.source);
                let Some(item) = find_completion_item(&items, tc.label) else {
                    t.fatal(format!("completion {:?} not found", tc.label));
                };
                let got = new_text(item);
                if got != tc.new_text {
                    t.error(format!("newText = {got:?}, want {:?}", tc.new_text));
                }
            });
        }
    });
}
