use std::sync::Arc;

use ets_shim::ast::{Kind, SourceFileParseOptions};
use ets_shim::core::context::Context;
use ets_shim::core::{CompilerOptions, ScriptKind, TextChange};
use ets_shim::ls::{change, lsconv, lsutil};
use ets_shim::lsproto;

// go test: internal/rewriter/tracker_test.go:18 TestNewPaths
// Rust: the empty-path subcase is left out: the Rust helpers panic on an empty path instead of returning nil
// (see Tracker.NewPropertyAccessPath).
#[test]
fn test_new_paths() {
    ets_go::testing::run(|t| {
        t.parallel();
        struct Case {
            name: &'static str,
            names: &'static [&'static str],
            type_name: bool,
            expected: &'static str,
        }
        let tests = [
            Case {
                name: "single expression",
                names: &["Effect"],
                type_name: false,
                expected: "const value: target = Effect",
            },
            Case {
                name: "expression path",
                names: &["X", "Effect", "map"],
                type_name: false,
                expected: "const value: target = X.Effect.map",
            },
            Case {
                name: "single type",
                names: &["Schema"],
                type_name: true,
                expected: "const value: Schema = target",
            },
            Case {
                name: "type path",
                names: &["X", "Schema", "Schema"],
                type_name: true,
                expected: "const value: X.Schema.Schema = target",
            },
        ];
        for test in tests {
            t.run(test.name, |t| {
                t.parallel();
                let source = "const value: target = target";
                let sf = ets_shim::parser::parse_source_file(
                    SourceFileParseOptions {
                        file_name: "/test.ts".to_string(),
                        ..Default::default()
                    },
                    source,
                    ScriptKind::TS,
                );
                let declaration = sf.statements().nodes()[0]
                    .as_variable_statement()
                    .declaration_list()
                    .as_variable_declaration_list()
                    .declarations()
                    .nodes()[0]
                    .as_variable_declaration();
                let converters =
                    lsconv::new_converters(lsproto::PositionEncodingKind::UTF16, move |_: &str| {
                        Some(lsconv::compute_lsp_line_starts(source))
                    });
                let ctx = Context::background();
                let mut tracker = ets_rewriter::new_tracker(change::new_tracker(
                    &ctx,
                    &CompilerOptions::default(),
                    lsutil::get_default_format_code_settings(),
                    Arc::clone(&converters),
                ));
                if test.type_name {
                    let node = tracker.new_qualified_name_path(test.names);
                    if test.names.len() > 1 && node.kind() != Kind::QualifiedName {
                        t.fatal(format!("kind = {:?}, want QualifiedName", node.kind()));
                    }
                    let type_name = declaration
                        .type_()
                        .expect("the type annotation")
                        .as_type_reference_node()
                        .type_name();
                    tracker.replace_node(sf, Some(type_name), Some(node), None);
                } else {
                    let node = tracker.new_property_access_path(test.names);
                    if test.names.len() > 1 && node.kind() != Kind::PropertyAccessExpression {
                        t.fatal(format!("kind = {:?}, want PropertyAccessExpression", node.kind()));
                    }
                    tracker.replace_node(sf, declaration.initializer(), Some(node), None);
                }
                let mut changes = Vec::new();
                for edit in tracker.get_changes().shift_remove(sf.file_name()).unwrap_or_default() {
                    changes.push(TextChange {
                        text_range: converters.from_lsp_range_to_original(&sf, edit.range),
                        new_text: edit.new_text.clone(),
                    });
                }
                let got = ets_shim::core::apply_bulk_edits(source, &changes);
                if got != test.expected {
                    t.fatal(format!("replacement = {got:?}, want {:?}", test.expected));
                }
            });
        }
    });
}
