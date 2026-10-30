use crate::*;

// go test: internal/completions/error_key_patterns_test.go:5 errorKeyPatternTsConfig 515bd0bc82d68b0f
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

// go test: internal/completions/error_key_patterns_test.go:23 taggedClassKeyPatternTsConfig 47f671e7f9868a50
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

// go test: internal/completions/error_key_patterns_test.go:40 TestErrorKeyPatterns 56bbf248824ff8df
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
                    package_json = HARNESS_PACKAGE_JSON;
                }
                let items =
                    completion_items_in(t, tc.tsconfig, package_json, tc.source, tc.source.len());
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
