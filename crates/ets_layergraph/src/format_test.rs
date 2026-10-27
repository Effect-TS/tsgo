use ets_go::sprintf;

use crate::format::{mermaid_entity_encode, mermaid_safe};

// go test: internal/layergraph/format_test.go:7 TestMermaidEntityEncode d59f62af6b55941c
#[test]
fn test_mermaid_entity_encode() {
    ets_go::testing::run(|t| {
        t.parallel();
        let tests: &[(&str, &str, &str)] = &[
            ("empty string", "", ""),
            ("no special chars", "hello world", "hello world"),
            ("quotes", r#""hello""#, "#quot;hello#quot;"),
            ("angle brackets", "<div>", "#lt;div#gt;"),
            ("parentheses", "fn(x)", "fn#40;x#41;"),
            ("curly braces", r#"{"key":"value"}"#, "#123;#quot;key#quot;:#quot;value#quot;#125;"),
            ("hash character", "color#red", "color#35;red"),
            ("no double encoding", "#", "#35;"),
            (
                "complex type with special chars",
                r#"IsGeneric<"With<Special>Chars#!">"#,
                "IsGeneric#lt;#quot;With#lt;Special#gt;Chars#35;!#quot;#gt;",
            ),
            (
                "JSON edge label",
                r#"{"relationship":"pipe"}"#,
                "#123;#quot;relationship#quot;:#quot;pipe#quot;#125;",
            ),
            (
                "JSON edge label with argumentIndex",
                r#"{"relationship":"call","argumentIndex":0}"#,
                "#123;#quot;relationship#quot;:#quot;call#quot;,#quot;argumentIndex#quot;:0#125;",
            ),
        ];

        for &(name, input, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let result = mermaid_entity_encode(input);
                if result != expected {
                    t.error(sprintf!(
                        "mermaidEntityEncode(%q) = %q, want %q",
                        input,
                        result,
                        expected
                    ));
                }
            });
        }
    });
}

// go test: internal/layergraph/format_test.go:82 TestMermaidSafe d4c1d2a407caed84
#[test]
fn test_mermaid_safe() {
    ets_go::testing::run(|t| {
        t.parallel();
        let tests: &[(&str, &str, &str)] = &[
            ("simple text", "hello", "hello"),
            ("newlines replaced with space", "hello\nworld", "hello world"),
            ("whitespace collapsed", "hello   world", "hello world"),
            (
                "truncated to 50 chars",
                "a234567890b234567890c234567890d234567890e234567890EXTRA",
                "a234567890b234567890c234567890d234567890e234567890",
            ),
            ("quotes encoded", r#"say "hello""#, "say #quot;hello#quot;"),
            ("angle brackets encoded", "List<number>", "List#lt;number#gt;"),
            ("hash NOT encoded", "color#red", "color#red"),
            (
                "special chars type",
                r#"IsGeneric<"With<Special>Chars#!">"#,
                "IsGeneric#lt;#quot;With#lt;Special#gt;Chars#!#quot;#gt;",
            ),
            ("trimmed", "  hello  ", "hello"),
        ];

        for &(name, input, expected) in tests {
            t.run(name, |t| {
                t.parallel();
                let result = mermaid_safe(input);
                if result != expected {
                    t.error(sprintf!("mermaidSafe(%q) = %q, want %q", input, result, expected));
                }
            });
        }
    });
}
