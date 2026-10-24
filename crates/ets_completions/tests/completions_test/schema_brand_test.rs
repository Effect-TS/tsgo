use crate::*;

// go test: internal/completions/schema_brand_test.go:7 TestSchemaBrand_EmptySource
#[test]
fn test_schema_brand_empty_source() {
    go_test(|t| {
        t.parallel();
        let source = "";
        let items = schema_brand_items(t, source, 0);
        if !items.is_empty() {
            t.error(format!("expected nil for empty source, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/schema_brand_test.go:16 TestSchemaBrand_CursorAtStartOfFile
#[test]
fn test_schema_brand_cursor_at_start_of_file() {
    go_test(|t| {
        t.parallel();
        let source = "const x = 1";
        let items = schema_brand_items(t, source, 0);
        if !items.is_empty() {
            t.error(format!("expected nil for cursor at start of file, got {} items", items.len()));
        }
    });
}

// go test: internal/completions/schema_brand_test.go:25 TestSchemaBrand_InsideImportDeclaration
#[test]
fn test_schema_brand_inside_import_declaration() {
    go_test(|t| {
        t.parallel();
        // Cursor inside an import declaration should not trigger completion
        let source = r#"import { Schema } from "effect""#;
        let pos = "import { Schema".len();
        let items = schema_brand_items(t, source, pos);
        if !items.is_empty() {
            t.error(format!(
                "expected nil for cursor inside import declaration, got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/schema_brand_test.go:36 TestSchemaBrand_CursorAfterNumber
#[test]
fn test_schema_brand_cursor_after_number() {
    go_test(|t| {
        t.parallel();
        // Cursor after a non-identifier token — ParseAccessedExpressionForCompletion returns nil
        let source = "const x = 42";
        let items = schema_brand_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for cursor after number literal, got {} items",
                items.len()
            ));
        }
    });
}

// go test: internal/completions/schema_brand_test.go:46 TestSchemaBrand_CursorAfterString
#[test]
fn test_schema_brand_cursor_after_string() {
    go_test(|t| {
        t.parallel();
        // Cursor after a string literal — not a dot-access or identifier
        let source = r#"const x = "hello""#;
        let items = schema_brand_items(t, source, source.len());
        if !items.is_empty() {
            t.error(format!(
                "expected nil for cursor after string literal, got {} items",
                items.len()
            ));
        }
    });
}
