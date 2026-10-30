use ets_go::testing::T;
use ets_shim::lsproto;

use crate::*;

/// Rust-only: an element of Go's anonymous `snippets` struct slice.
pub(crate) struct Snippet {
    api: &'static str,
    new_text: &'static str,
}

/// Rust-only: an element of Go's anonymous `schemaClassCompletionCases` struct slice.
pub(crate) struct SchemaClassCompletionCase {
    version: &'static str,
    directive: &'static str,
    snippets: &'static [Snippet],
}

// go test: internal/completions/effect_schema_self_in_classes_test.go:15 schemaClassCompletionCases 05add3c6f7aa2a37
static SCHEMA_CLASS_COMPLETION_CASES: &[SchemaClassCompletionCase] = &[
    SchemaClassCompletionCase {
        version: "v4",
        directive: "",
        snippets: &[
            Snippet {
                api: "Class",
                new_text: r#"Class<Foo, { readonly brand: unique symbol }>("Foo")({${0}}){}"#,
            },
            Snippet {
                api: "Error",
                new_text: r#"Error<Foo, { readonly brand: unique symbol }>("Foo")({${0}}){}"#,
            },
            Snippet {
                api: "Opaque",
                new_text: r"Opaque<Foo, { readonly brand: unique symbol }>()(${0}){}",
            },
            Snippet {
                api: "TaggedClass",
                new_text: r#"TaggedClass<Foo, { readonly brand: unique symbol }>()("Foo", {${0}}){}"#,
            },
            Snippet {
                api: "TaggedError",
                new_text: r#"TaggedError<Foo, { readonly brand: unique symbol }>()("Foo", {${0}}){}"#,
            },
        ],
    },
    SchemaClassCompletionCase {
        version: "v3",
        directive: "// @effect-v3\n",
        snippets: &[
            Snippet { api: "Class", new_text: r#"Class<Foo>("Foo")({${0}}){}"# },
            Snippet { api: "TaggedClass", new_text: r#"TaggedClass<Foo>()("Foo", {${0}}){}"# },
            Snippet { api: "TaggedError", new_text: r#"TaggedError<Foo>()("Foo", {${0}}){}"# },
            Snippet { api: "TaggedRequest", new_text: r#"TaggedRequest<Foo>()("Foo", {${0}}){}"# },
        ],
    },
];

// go test: internal/completions/effect_schema_self_in_classes_test.go:51 schemaClassCompletionItems d3b7726a6c38e378
fn schema_class_completion_items(
    t: &T,
    source: &str,
    position: usize,
) -> Vec<lsproto::CompletionItem> {
    t.helper();
    filter_completion_items(completion_items_at(t, source, position), |item| {
        item.label.ends_with("<Foo>")
    })
}

/// Rust-only: Go's `item == nil || item.TextEdit == nil || item.TextEdit.TextEdit == nil`, negated.
fn has_text_edit(item: Option<&lsproto::CompletionItem>) -> bool {
    item.is_some_and(|item| item.text_edit.as_ref().is_some_and(|e| e.text_edit.is_some()))
}

// go test: internal/completions/effect_schema_self_in_classes_test.go:58 TestEffectSchemaSelfInClasses_ModuleImports 43b3d6c1357c0135
#[test]
fn test_effect_schema_self_in_classes_module_imports() {
    go_test(|t| {
        t.parallel();
        struct Module {
            name: &'static str,
            import_text: &'static str,
            identifier: &'static str,
            member: &'static str,
        }
        for version in SCHEMA_CLASS_COMPLETION_CASES {
            for module in [
                Module {
                    name: "barrel",
                    import_text: r#"import { Schema } from "effect""#,
                    identifier: "Schema",
                    member: "",
                },
                Module {
                    name: "namespace",
                    import_text: r#"import * as Schema from "effect/Schema""#,
                    identifier: "Schema",
                    member: "",
                },
                Module {
                    name: "barrel alias",
                    import_text: r#"import { Schema as S } from "effect""#,
                    identifier: "S",
                    member: "",
                },
                Module {
                    name: "namespace alias",
                    import_text: r#"import * as S from "effect/Schema""#,
                    identifier: "S",
                    member: "",
                },
                Module {
                    name: "partial member",
                    import_text: r#"import { Schema } from "effect""#,
                    identifier: "Schema",
                    member: "Tagg",
                },
            ] {
                t.run(&format!("{}/{}", version.version, module.name), |t| {
                    t.parallel();
                    let source = format!(
                        "{}{}\nclass Foo extends {}.{}",
                        version.directive, module.import_text, module.identifier, module.member
                    );
                    let items = schema_class_completion_items(t, &source, source.len());
                    if items.len() != version.snippets.len() {
                        t.error(format!(
                            "got {} custom items, want {}",
                            items.len(),
                            version.snippets.len()
                        ));
                    }
                    for (i, expected) in version.snippets.iter().enumerate() {
                        let label = format!("{}<Foo>", expected.api);
                        let item = find_completion_item(&items, &label);
                        if !has_text_edit(item) {
                            t.error(format!("missing snippet text edit for {label:?}"));
                            continue;
                        }
                        let got = new_text(item.unwrap());
                        let want = format!("{}.{}", module.identifier, expected.new_text);
                        if got != want {
                            t.error(format!("{label} NewText = {got:?}, want {want:?}"));
                        }
                        if i < items.len() && items[i].label != label {
                            t.error(format!(
                                "item[{i}].Label = {:?}, want {label:?}",
                                items[i].label
                            ));
                        }
                    }
                });
            }
        }
    });
}

// go test: internal/completions/effect_schema_self_in_classes_test.go:99 TestEffectSchemaSelfInClasses_DirectImports 2a695a839c6452b8
#[test]
fn test_effect_schema_self_in_classes_direct_imports() {
    go_test(|t| {
        t.parallel();
        for version in SCHEMA_CLASS_COMPLETION_CASES {
            for expected in version.snippets {
                t.run(&format!("{}/{}", version.version, expected.api), |t| {
                    t.parallel();
                    let source = format!(
                        "{}import {{ {} }} from \"effect/Schema\"\nclass Foo extends {}",
                        version.directive, expected.api, expected.api
                    );
                    let items = schema_class_completion_items(t, &source, source.len());
                    if items.len() != 1 {
                        t.fatal(format!("got {} custom items, want 1", items.len()));
                    }
                    let item = &items[0];
                    if item.label != format!("{}<Foo>", expected.api) || !has_text_edit(Some(item))
                    {
                        t.fatal(format!(
                            "expected {}<Foo> snippet text edit, got {item:?}",
                            expected.api
                        ));
                    }
                    let got = new_text(item);
                    if got != expected.new_text {
                        t.error(format!("NewText = {got:?}, want {:?}", expected.new_text));
                    }
                });
            }
        }
    });
}

// go test: internal/completions/effect_schema_self_in_classes_test.go:122 TestEffectSchemaSelfInClasses_MetadataAndRange 5e08bc5b89ae1369
#[test]
fn test_effect_schema_self_in_classes_metadata_and_range() {
    go_test(|t| {
        t.parallel();
        let source = r#"import { Schema } from "effect"
class Foo extends Schema.Tagg
const after = 1"#;
        let position = source.find("Schema.Tagg").unwrap() + "Schema.Tagg".len();
        let items = schema_class_completion_items(t, source, position);
        let item = find_completion_item(&items, "TaggedClass<Foo>");
        let kind = lsproto::CompletionItemKind::VARIABLE;
        let format = lsproto::InsertTextFormat::SNIPPET;
        let sort_text = "11".to_string();
        let filter_text = "Schema.TaggedClass<Foo>".to_string();
        let expected = lsproto::CompletionItem {
            label: "TaggedClass<Foo>".to_string(),
            kind: Some(kind),
            insert_text_format: Some(format),
            sort_text: Some(sort_text),
            filter_text: Some(filter_text),
            text_edit: Some(lsproto::TextEditOrInsertReplaceEdit {
                text_edit: Some(lsproto::TextEdit {
                    new_text: r#"Schema.TaggedClass<Foo, { readonly brand: unique symbol }>()("Foo", {${0}}){}"#
                        .to_string(),
                    range: lsproto::Range {
                        start: lsproto::Position { line: 1, character: 18 },
                        end: lsproto::Position { line: 1, character: 29 },
                    },
                }),
                ..Default::default()
            }),
            data: Some(lsproto::CompletionItemData {
                file_name: "/test.ts".to_string(),
                position: 61,
                name: "TaggedClass<Foo>".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        };
        // Go's `cmp.Diff(expected, item)`.
        if item != Some(&expected) {
            t.error(format!("completion mismatch (-want +got):\n- {expected:?}\n+ {item:?}"));
        }
    });
}

// go test: internal/completions/effect_schema_self_in_classes_test.go:160 TestEffectSchemaSelfInClasses_ModelUnchanged ae05004428c89881
#[test]
fn test_effect_schema_self_in_classes_model_unchanged() {
    go_test(|t| {
        t.parallel();
        for (source, new_text_want) in [
            (
                "import { Model } from \"effect/schema\"\nclass Foo extends Model.",
                r#"Model.Class<Foo>("Foo")({${0}}){}"#,
            ),
            (
                "import * as Model from \"effect/schema/Model\"\nclass Foo extends Model.",
                r#"Model.Class<Foo>("Foo")({${0}}){}"#,
            ),
            (
                "import { Class } from \"effect/schema/Model\"\nclass Foo extends Class",
                r#"Class<Foo>("Foo")({${0}}){}"#,
            ),
        ] {
            let items = schema_class_completion_items(t, source, source.len());
            if items.len() != 1 || !has_text_edit(items.first()) {
                t.fatal(format!("expected one Model snippet text edit, got {items:?}"));
            }
            let got = new_text(&items[0]);
            if got != new_text_want {
                t.error(format!("NewText = {got:?}, want {new_text_want:?}"));
            }
        }
    });
}

// go test: internal/completions/effect_schema_self_in_classes_test.go:180 TestEffectSchemaSelfInClasses_OutsideSchemaExtends ebdf0b57e71917df
#[test]
fn test_effect_schema_self_in_classes_outside_schema_extends() {
    go_test(|t| {
        t.parallel();
        let positive = "import { Schema } from \"effect\"\nclass Foo extends Schema.";
        let items = schema_class_completion_items(t, positive, positive.len());
        if items.len() != 5 {
            t.fatal(format!("got {} Schema extends snippets, want 5", items.len()));
        }
        for source in [
            "import { Schema } from \"effect\"\nconst value = Schema.",
            "import { Schema } from \"effect\"\ninterface Foo extends Schema.Class",
            r#"import { Schema } from "effect"
namespace Local {
  export declare function Class(): void
  export declare function TaggedClass(): void
  export declare function TaggedError(): void
  export declare function Error(): void
  export declare function Opaque(): void
}
class Foo extends Local."#,
        ] {
            let items = schema_class_completion_items(t, source, source.len());
            if !items.is_empty() {
                t.error(format!("unexpected Schema extends snippets for {source:?}: {items:?}"));
            }
        }
    });
}

// go test: internal/completions/effect_schema_self_in_classes_test.go:205 TestEffectSchemaSelfInClasses_AcceptedSnippet 976744b9a8b64b8e
#[test]
fn test_effect_schema_self_in_classes_accepted_snippet() {
    go_test(|t| {
        t.parallel();
        for expected in SCHEMA_CLASS_COMPLETION_CASES[0].snippets {
            t.run(expected.api, |t| {
                t.parallel();
                let config = COMPLETION_TEST_TS_CONFIG.replacen(
                    r#""name": "@effect/language-service""#,
                    r#""name": "@effect/language-service", "diagnosticSeverity": { "schemaClassMissingBrand": "error" }"#,
                    1,
                );
                const SOURCE: &str = "import { Schema } from \"effect\"\nclass Foo extends Schema.";
                let content = format!(
                    "// @Filename: /tsconfig.json\n{config}\n// @Filename: /test.ts\n{SOURCE}/*1*/"
                );
                let mut f = ets_fourslash::new_fourslash(t, None, &content);
                f.go_to_marker(t, "1");
                let Some(completions) = f.get_completions(t, None) else {
                    t.fatal("missing completions");
                };
                let item = find_completion_item(&completions.items, &format!("{}<Foo>", expected.api));
                if !has_text_edit(item) {
                    t.fatal("missing snippet text edit");
                }
                let item = item.unwrap();
                let mut placeholder = "";
                if expected.api == "Opaque" {
                    placeholder = "Schema.Struct({})";
                }
                let snippet = new_text(item).replace("${0}", placeholder);
                let edit_range = edit_range(item);
                let line_start = SOURCE.find('\n').unwrap() + 1;
                let start = line_start as i64 + i64::from(edit_range.start.character);
                let length = i64::from(edit_range.end.character - edit_range.start.character);
                f.replace(t, start, length, &snippet);
                f.verify_no_errors(t);
                let unbranded = snippet.replacen(", { readonly brand: unique symbol }", "", 1);
                f.replace(t, start, snippet.len() as i64, &unbranded);
                f.verify_number_of_errors_in_current_file(t, 1);
                f.done(t);
            });
        }
    });
}

// go test: internal/completions/effect_schema_self_in_classes_test.go:242 TestEffectSchemaSelfInClasses_BrandedSchemaClasses 71125da6a3714a62
#[test]
fn test_effect_schema_self_in_classes_branded_schema_classes() {
    go_test(|t| {
        t.parallel();
        const BRAND: &str = ", { readonly brand: unique symbol }";
        /// Rust-only: an element of Go's anonymous test case struct slice.
        struct Case {
            name: &'static str,
            options: &'static str,
            branded: &'static [&'static str],
        }
        for test in [
            Case {
                name: "reduced",
                options: r#""brandedSchemaClasses": ["Class", "Error", "Opaque"]"#,
                branded: &["Class", "Error", "Opaque"],
            },
            Case { name: "empty", options: r#""brandedSchemaClasses": []"#, branded: &[] },
            Case {
                name: "override",
                options: r#""brandedSchemaClasses": [], "overrides": [{ "include": ["test.ts"], "options": { "brandedSchemaClasses": ["TaggedError"] } }]"#,
                branded: &["TaggedError"],
            },
        ] {
            t.run(test.name, |t| {
                t.parallel();
                let config = COMPLETION_TEST_TS_CONFIG.replacen(
                    r#""name": "@effect/language-service""#,
                    &format!(r#""name": "@effect/language-service", {}"#, test.options),
                    1,
                );
                const SOURCE: &str = "import { Schema } from \"effect\"\nclass Foo extends Schema.";
                let items = completion_items_in(t, &config, "", SOURCE, SOURCE.len());
                for expected in SCHEMA_CLASS_COMPLETION_CASES[0].snippets {
                    let item = find_completion_item(&items, &format!("{}<Foo>", expected.api));
                    if !has_text_edit(item) {
                        t.error(format!("missing snippet text edit for {}<Foo>", expected.api));
                        continue;
                    }
                    let mut want = format!("Schema.{}", expected.new_text);
                    if !test.branded.contains(&expected.api) {
                        want = want.replacen(BRAND, "", 1);
                    }
                    let got = new_text(item.unwrap());
                    if got != want {
                        t.error(format!("{} NewText = {got:?}, want {want:?}", expected.api));
                    }
                }
            });
        }
    });
}
