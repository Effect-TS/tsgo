use crate::*;

// go test: internal/completions/duration_input_test.go:7 durationUnits 7ce86c2b92f72451
const DURATION_UNITS: &[&str] =
    &["nanos", "micros", "millis", "seconds", "minutes", "hours", "days", "weeks"];

// go test: internal/completions/duration_input_test.go:9 TestDurationInput_NotInsideStringContent 200e8df317663445
#[test]
fn test_duration_input_not_inside_string_content() {
    go_test(|t| {
        t.parallel();
        for (name, source, cursor) in [
            ("numeric literal", "const x = 123", ""),
            ("identifier", "const foo = bar", ""),
            ("opening quote", r#"const x: string = "hello""#, "const x: string = "),
            ("closing quote of terminated literal", r#"const x: string = "hello""#, ""),
            ("empty source", "", ""),
            ("comment", "// some comment", ""),
            ("before string literal", r#"const x = "hello""#, "const x ="),
        ] {
            t.run(name, |t| {
                t.parallel();
                let mut position = source.len();
                if !cursor.is_empty() {
                    position = cursor.len();
                }
                let items = labeled_items(t, source, position, DURATION_UNITS);
                if !items.is_empty() {
                    t.error(format!("expected no duration completions, got {} items", items.len()));
                }
            });
        }
    });
}

// go test: internal/completions/duration_input_test.go:37 TestDurationInput_DurationUnitsCount 97d38eacc00aafa0
#[test]
fn test_duration_input_duration_units_count() {
    go_test(|t| {
        t.parallel();
        let prefix = "type D = `${number} nanos` | `${number} micros` | `${number} millis` | `${number} seconds` | `${number} minutes` | `${number} hours` | `${number} days` | `${number} weeks`\nconst value: D = \"";
        let source = format!("{prefix}\"");
        let items = labeled_items(t, &source, prefix.len(), DURATION_UNITS);
        if items.len() != DURATION_UNITS.len() {
            t.fatal(format!(
                "expected {} duration completions, got {}",
                DURATION_UNITS.len(),
                items.len()
            ));
        }
        for unit in DURATION_UNITS {
            if find_completion_item(&items, unit).is_none() {
                t.error(format!("expected completion label {unit:?}"));
            }
        }
    });
}
