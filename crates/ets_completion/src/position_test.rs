use ets_go::strconv::quote;
use ets_shim::ast::{SourceFile, SourceFileParseOptions};
use ets_shim::core::{P, ScriptKind};
use ets_shim::scanner;

use crate::*;

// go test: internal/completion/position_test.go:12 parseSource
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

// go test: internal/completion/position_test.go:18 TestParseDataForExtendsClassCompletion_AfterDot
#[test]
fn test_parse_data_for_extends_class_completion_after_dot() {
    ets_go::testing::run(|t| {
        t.parallel();
        // class Foo extends Schema.| (cursor after the dot)
        let source = "class Foo extends Schema.";
        let sf = parse_source(source);
        let pos = len(source);

        let Some(data) = parse_data_for_extends_class_completion(sf, pos) else {
            t.fatal("expected non-nil result for 'class Foo extends Schema.'");
        };
        let got = data.accessed_object_text();
        if got != "Schema" {
            t.error(format!("AccessedObjectText = {}, want {}", quote(&got), quote("Schema")));
        }
        let got = data.class_name_text();
        if got != "Foo" {
            t.error(format!("ClassNameText = {}, want {}", quote(&got), quote("Foo")));
        }
    });
}

// go test: internal/completion/position_test.go:37 TestParseDataForExtendsClassCompletion_AfterPropertyName
#[test]
fn test_parse_data_for_extends_class_completion_after_property_name() {
    ets_go::testing::run(|t| {
        t.parallel();
        // class Foo extends Schema.Class| (cursor after Class)
        let source = "class Foo extends Schema.Class";
        let sf = parse_source(source);
        let pos = len(source);

        let Some(data) = parse_data_for_extends_class_completion(sf, pos) else {
            t.fatal("expected non-nil result for 'class Foo extends Schema.Class'");
        };
        let got = data.accessed_object_text();
        if got != "Schema" {
            t.error(format!("AccessedObjectText = {}, want {}", quote(&got), quote("Schema")));
        }
        let got = data.class_name_text();
        if got != "Foo" {
            t.error(format!("ClassNameText = {}, want {}", quote(&got), quote("Foo")));
        }
    });
}

// go test: internal/completion/position_test.go:56 TestParseDataForExtendsClassCompletion_StandaloneIdentifier
#[test]
fn test_parse_data_for_extends_class_completion_standalone_identifier() {
    ets_go::testing::run(|t| {
        t.parallel();
        // class Foo extends Schema| (cursor after standalone identifier)
        let source = "class Foo extends Schema";
        let sf = parse_source(source);
        let pos = len(source);

        let Some(data) = parse_data_for_extends_class_completion(sf, pos) else {
            t.fatal("expected non-nil result for 'class Foo extends Schema'");
        };
        let got = scanner::get_text_of_node(data.accessed_object);
        if got != "Schema" {
            t.error(format!("AccessedObject text = {}, want {}", quote(&got), quote("Schema")));
        }
        let got = data.class_name_text();
        if got != "Foo" {
            t.error(format!("ClassNameText = {}, want {}", quote(&got), quote("Foo")));
        }
    });
}

// go test: internal/completion/position_test.go:76 TestParseDataForExtendsClassCompletion_NonClassContext
#[test]
fn test_parse_data_for_extends_class_completion_non_class_context() {
    ets_go::testing::run(|t| {
        t.parallel();
        // Not in a class extends clause
        let source = "const x = Schema.Class";
        let sf = parse_source(source);
        let pos = len(source);

        let data = parse_data_for_extends_class_completion(sf, pos);
        if data.is_some() {
            t.error("expected nil result for non-class context");
        }
    });
}

// go test: internal/completion/position_test.go:89 TestParseDataForExtendsClassCompletion_InsideImportDeclaration
#[test]
fn test_parse_data_for_extends_class_completion_inside_import_declaration() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = r#"import { Schema } from "effect""#;
        let sf = parse_source(source);
        // Position after "Schema" inside the import
        let pos = len("import { Schema");

        let data = parse_data_for_extends_class_completion(sf, pos);
        if data.is_some() {
            t.error("expected nil result for position inside import declaration");
        }
    });
}

// go test: internal/completion/position_test.go:102 TestParseDataForExtendsClassCompletion_ReplacementSpan
#[test]
fn test_parse_data_for_extends_class_completion_replacement_span() {
    ets_go::testing::run(|t| {
        t.parallel();
        let source = "class Foo extends Schema.Class";
        let sf = parse_source(source);
        let pos = len(source);

        let Some(data) = parse_data_for_extends_class_completion(sf, pos) else {
            t.fatal("expected non-nil result");
        };
        // The replacement span should cover "Schema.Class"
        if data.replacement_length == 0 {
            t.error(format!(
                "expected positive ReplacementLength, got {}",
                data.replacement_length
            ));
        }
        let start = data.replacement_start as usize;
        let replaced = &source[start..start + data.replacement_length as usize];
        if replaced != "Schema.Class" {
            t.error(format!(
                "replacement span covers {}, want {}",
                quote(replaced),
                quote("Schema.Class")
            ));
        }
    });
}
