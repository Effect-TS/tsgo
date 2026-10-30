use ets_go::strconv::quote;
use ets_shim::ast::{SourceFile, SourceFileParseOptions};
use ets_shim::core::{P, ScriptKind};

use crate::*;

// go test: internal/completion/position_test.go:11 parseSource f4cd2f032a49fd07
fn parse_source(source: &str) -> P<SourceFile> {
    ets_shim::parser::parse_source_file(
        SourceFileParseOptions { file_name: "/test.ts".to_string(), ..Default::default() },
        source,
        ScriptKind::TS,
    )
}

/// Rust-only: Go's `len(source)` as a position.
fn len(source: &str) -> u32 {
    u32::try_from(source.len()).unwrap()
}

// go test: internal/completion/position_test.go:17 TestParseDataForExtendsClassCompletion 00faa2725d89f1b8
#[test]
fn test_parse_data_for_extends_class_completion() {
    ets_go::testing::run(|t| {
        t.parallel();
        for (source, want_object, want_replaced) in [
            ("class Foo extends Schema.", "Schema", "Schema."),
            ("class Foo extends X.Schema.", "X.Schema", "X.Schema."),
            ("class Foo extends Schema.Class", "Schema", "Schema.Class"),
            ("class Foo extends Schema", "Schema", "Schema"),
        ] {
            t.run(source, |t| {
                t.parallel();
                let Some(data) =
                    parse_data_for_extends_class_completion(parse_source(source), len(source))
                else {
                    t.fatal(format!("expected non-nil result for {}", quote(source)));
                };
                let got = data.accessed_object_text();
                if got != want_object {
                    t.error(format!(
                        "AccessedObjectText = {}, want {}",
                        quote(&got),
                        quote(want_object)
                    ));
                }
                let got = data.class_name_text();
                if got != "Foo" {
                    t.error(format!("ClassNameText = {}, want {}", quote(&got), quote("Foo")));
                }
                let start = data.replacement_start as usize;
                let got = &source[start..start + data.replacement_length as usize];
                if got != want_replaced {
                    t.error(format!(
                        "replacement span covers {}, want {}",
                        quote(got),
                        quote(want_replaced)
                    ));
                }
            });
        }
    });
}

// go test: internal/completion/position_test.go:48 TestParseDataForExtendsClassCompletion_Rejected 2494facc59a84dcf
#[test]
fn test_parse_data_for_extends_class_completion_rejected() {
    ets_go::testing::run(|t| {
        t.parallel();
        for (name, source, position) in [
            ("non-class context", "const x = Schema.Class", len("const x = Schema.Class")),
            (
                "inside import declaration",
                r#"import { Schema } from "effect""#,
                len("import { Schema"),
            ),
        ] {
            t.run(name, |t| {
                t.parallel();
                if parse_data_for_extends_class_completion(parse_source(source), position).is_some()
                {
                    t.error(format!("expected nil result for {} at {}", quote(source), position));
                }
            });
        }
    });
}
