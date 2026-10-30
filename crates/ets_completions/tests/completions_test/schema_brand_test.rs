use crate::*;

// go test: internal/completions/schema_brand_test.go:10 TestSchemaBrand 87c3d2fd79d07944
#[test]
fn test_schema_brand() {
    go_test(|t| {
        t.parallel();
        const V3_SCHEMA: &str = "// @effect-v3\nimport { Schema } from \"effect\"\n";
        for (name, source, cursor, want) in [
            (
                "v3 variable declaration",
                format!("{V3_SCHEMA}const UserId = Schema."),
                "",
                r#"brand("UserId")"#,
            ),
            (
                "v4 is unsupported",
                "import { Schema } from \"effect\"\nconst UserId = Schema.".to_string(),
                "",
                "",
            ),
            ("outside variable declaration", format!("{V3_SCHEMA}Schema."), "", ""),
            ("inside import declaration", V3_SCHEMA.to_string(), "import { Schema", ""),
            ("after number literal", format!("{V3_SCHEMA}const x = 42"), "", ""),
            ("after string literal", format!(r#"{V3_SCHEMA}const x = "hello""#), "", ""),
            ("empty source", String::new(), "", ""),
        ] {
            t.run(name, |t| {
                t.parallel();
                let mut position = source.len();
                if !cursor.is_empty() {
                    position = source.find(cursor).unwrap() + cursor.len();
                }
                let items =
                    filter_completion_items(completion_items_at(t, &source, position), |item| {
                        item.label.starts_with(r#"brand(""#)
                    });
                if want.is_empty() {
                    if !items.is_empty() {
                        t.error(format!(
                            "expected no brand completions, got {} items",
                            items.len()
                        ));
                    }
                    return;
                }
                if items.len() != 1 || items[0].label != want {
                    t.fatal(format!("expected single {want:?} completion, got {items:?}"));
                }
            });
        }
    });
}
