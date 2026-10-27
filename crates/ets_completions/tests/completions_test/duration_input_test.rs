use crate::*;

// go test: internal/completions/duration_input_test.go:7 TestDurationInput_CursorOutsideString 2d70b1313b8f4234
#[test]
fn test_duration_input_cursor_outside_string() {
    go_test(|t| {
        t.parallel();
        // Cursor is on a numeric literal, not a string
        let source = "const x = 123";
        let items = duration_input_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!("expected nil for cursor outside string, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/duration_input_test.go:17 TestDurationInput_CursorOnIdentifier ff7e6cd714a84595
#[test]
fn test_duration_input_cursor_on_identifier() {
    go_test(|t| {
        t.parallel();
        // Cursor is on an identifier, not inside a string
        let source = "const foo = bar";
        let items = duration_input_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!("expected nil for cursor on identifier, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/duration_input_test.go:27 TestDurationInput_CursorAtOpeningQuote 6d375ea2a6a10034
#[test]
fn test_duration_input_cursor_at_opening_quote() {
    go_test(|t| {
        t.parallel();
        // Cursor is at the position of the opening quote (not inside the string content)
        let source = r#"const x: string = "hello""#;
        // Position the cursor at the opening quote character
        let pos = "const x: string = ".len();
        let items = duration_input_items(t, source, pos);
        if !items.is_empty() {
            t.error(format!("expected nil for cursor at opening quote, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/duration_input_test.go:39 TestDurationInput_CursorAtClosingQuote c63ce8493aecff75
#[test]
fn test_duration_input_cursor_at_closing_quote() {
    go_test(|t| {
        t.parallel();
        // Cursor at the closing quote position (end of node) for a terminated literal → not inside
        let source = r#"const x: string = "hello""#;
        // Position at the closing quote
        let pos = r#"const x: string = "hello""#.len();
        let items = duration_input_items(t, source, pos);
        if !items.is_empty() {
            t.error(format!(
                "expected nil for cursor at closing quote of terminated literal, got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/duration_input_test.go:51 TestDurationInput_EmptySource f876f02fb4fe5c16
#[test]
fn test_duration_input_empty_source() {
    go_test(|t| {
        t.parallel();
        let source = "";
        let items = duration_input_items(t, source, 0);
        if !items.is_empty() {
            t.error(format!("expected nil for empty source, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/duration_input_test.go:60 TestDurationInput_CursorInComment aca5200d1aa968ea
#[test]
fn test_duration_input_cursor_in_comment() {
    go_test(|t| {
        t.parallel();
        // Cursor is inside a comment, not a string
        let source = "// some comment";
        let items = duration_input_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!("expected nil for cursor in comment, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/duration_input_test.go:70 TestDurationInput_CursorBeforeString fd2be3574fc133b6
#[test]
fn test_duration_input_cursor_before_string() {
    go_test(|t| {
        t.parallel();
        // Cursor is positioned before the string literal (on the equals sign)
        let source = r#"const x = "hello""#;
        let pos = "const x =".len();
        let items = duration_input_items(t, source, pos);
        if !items.is_empty() {
            t.error(format!(
                "expected nil for cursor before string literal, got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/duration_input_test.go:81 TestDurationInput_DurationUnitsCount 55271b1a3b15cce8
#[test]
fn test_duration_input_duration_units_count() {
    go_test(|t| {
        t.parallel();
        // Verify that the durationUnits slice has exactly 8 units
        let expected =
            ["nanos", "micros", "millis", "seconds", "minutes", "hours", "days", "weeks"];
        let source = "type D = `${number} nanos` | `${number} micros` | `${number} millis` | `${number} seconds` | `${number} minutes` | `${number} hours` | `${number} days` | `${number} weeks`\nconst value: D = \"\"";
        let items = duration_input_items(
            t,
            source,
            "type D = `${number} nanos` | `${number} micros` | `${number} millis` | `${number} seconds` | `${number} minutes` | `${number} hours` | `${number} days` | `${number} weeks`\nconst value: D = \"".len(),
        );
        if items.len() != expected.len() {
            t.fatal(format!(
                "expected {} duration completions, got {}",
                expected.len(),
                items.len()
            ));
        }
        for unit in expected {
            if find_completion_item(&items, unit).is_none() {
                t.error(format!("expected completion label {unit:?}"));
            }
        }
    });
}
