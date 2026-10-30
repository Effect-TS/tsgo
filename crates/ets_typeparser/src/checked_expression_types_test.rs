// Rust: Go's test binary registers RecordCheckedExpressionType as the checker's expression callback
// (checked_expression_types_init_test.go); here the program-building helpers do (helpers_test.rs, `init`).

use ets_shim::core::context::Context;

use crate::helpers_test::*;

// go test: internal/typeparser/checked_expression_types_test.go:5 TestCheckedExpressionTypeRemembersOnlyCheckedSourceFiles 39b0dbabcdc30956
#[test]
fn test_checked_expression_type_remembers_only_checked_source_files() {
    run(|t| {
        t.parallel();

        let (mut compiled, files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[
                ("/.src/first.ts", "export const first = { value: 1 }"),
                ("/.src/second.ts", "export const second = { value: \"second\" }"),
            ],
        );
        let mut tp = compiled.type_parser();

        let (first_file, second_file) = (files["/.src/first.ts"], files["/.src/second.ts"]);
        let first = find_identifier_by_text(t, first_file, "first", 0)
            .parent()
            .and_then(|parent| parent.initializer())
            .expect("initializer of first");
        let second = find_identifier_by_text(t, second_file, "second", 0)
            .parent()
            .and_then(|parent| parent.initializer())
            .expect("initializer of second");

        if tp.checked_expression_type(first).is_some()
            || tp.checked_expression_type(second).is_some()
        {
            t.fatal("expected no checked types before any check");
        }
        if tp.links().checked_expression_type_source_file.is_some() {
            t.fatal("an unchecked source file was remembered");
        }

        tp.checker().get_diagnostics_exported(&Context::background(), first_file);
        let first_type = tp.checked_expression_type(first);
        if first_type
            .is_none_or(|first_type| type_string(&mut tp, first_type) != "{ value: number; }")
        {
            t.fatal(ets_go::sprintf!(
                "expected the checked type of first, got %v",
                first_type.map(|first_type| type_string(&mut tp, first_type))
            ));
        }
        if tp.links().checked_expression_type_source_file != Some(first_file) {
            t.fatal("expected the checked first file to be remembered");
        }

        if tp.checked_expression_type(second).is_some() {
            t.fatal("expected no checked type for the unchecked second file");
        }
        if tp.links().checked_expression_type_source_file != Some(first_file) {
            t.fatal("an unchecked query replaced the remembered source file");
        }
        tp.checker().get_diagnostics_exported(&Context::background(), second_file);
        let second_type = tp.checked_expression_type(second);
        if second_type
            .is_none_or(|second_type| type_string(&mut tp, second_type) != "{ value: string; }")
        {
            t.fatal(ets_go::sprintf!(
                "expected the checked type of second, got %v",
                second_type.map(|second_type| type_string(&mut tp, second_type))
            ));
        }
        if tp.links().checked_expression_type_source_file != Some(second_file) {
            t.fatal("expected the checked second file to be remembered");
        }

        if tp.checked_expression_type(first) != first_type
            || tp.checked_expression_type(second) != second_type
        {
            t.fatal("expected the same checked types after switching files");
        }
    });
}
