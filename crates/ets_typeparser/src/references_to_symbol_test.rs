// Ported from effect-tsgo's internal/typeparser/references_to_symbol_test.go.

use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::core::P;

use crate::helpers_test::*;
use crate::*;

// go test: internal/typeparser/references_to_symbol_test.go:15 identifierOccurrences 5eec38b4a05b779c
// identifierOccurrences maps each reference to its occurrence index among the
// identifiers named text in sf (the index findIdentifierByText takes), and
// checks that the references are identifiers in source order.
//
// Rust: Go's `-1` for a reference that is not found is `None`.
fn identifier_occurrences(
    t: &T,
    sf: P<SourceFile>,
    text: &str,
    references: &[P<Node>],
) -> Vec<Option<usize>> {
    t.helper();
    let mut occurrences = Vec::new();
    let mut previous_end = None;
    for &reference in references {
        if reference.kind() != Kind::Identifier || reference.text() != text {
            t.fatal(format!(
                "expected identifier {text:?}, got {:?} {:?}",
                reference.kind(),
                reference.text()
            ));
        }
        if previous_end.is_some_and(|end| reference.pos() < end) {
            t.fatal("references are not in source order");
        }
        previous_end = Some(reference.end());
        let mut occurrence = None;
        let mut i = 0;
        loop {
            let candidate = find_identifier_by_text(t, sf, text, i);
            if candidate == reference {
                occurrence = Some(i);
                break;
            }
            if candidate.pos() > reference.pos() {
                break;
            }
            i += 1;
        }
        occurrences.push(occurrence);
    }
    occurrences
}

// go test: internal/typeparser/references_to_symbol_test.go:43 expectOccurrences 3a914fa8dfe429d0
fn expect_occurrences(t: &T, got: &[Option<usize>], want: &[usize]) {
    t.helper();
    let want: Vec<Option<usize>> = want.iter().map(|&i| Some(i)).collect();
    if got != want.as_slice() {
        t.fatal(format!("expected references at occurrences {want:?}, got {got:?}"));
    }
}

// go test: internal/typeparser/references_to_symbol_test.go:50 TestReferencesToSymbol_Identifiers e702b8cc3b531490
#[test]
fn test_references_to_symbol_identifiers() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            "export function f(e: number) {\n  return e + e\n}",
        );
        let mut tp = compiled.type_parser();

        let Some(symbol) = tp.get_symbol_at_location(find_identifier_by_text(t, sf, "e", 0)) else {
            t.fatal("expected parameter symbol");
        };
        let references = tp.references_to_symbol(sf.as_node(), symbol);
        expect_occurrences(t, &identifier_occurrences(t, sf, "e", &references), &[0, 1, 2]);
    });
}

// go test: internal/typeparser/references_to_symbol_test.go:65 TestReferencesToSymbol_ShorthandProperty 3d189b0d546ef185
#[test]
fn test_references_to_symbol_shorthand_property() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            "export const f = (e: number) => ({ e, other: e })",
        );
        let mut tp = compiled.type_parser();

        let parameter = find_identifier_by_text(t, sf, "e", 0);
        let Some(symbol) = tp.get_symbol_at_location(parameter) else {
            t.fatal("expected parameter symbol");
        };
        let body = parameter.parent().and_then(|p| p.parent()).and_then(|f| f.body());
        let references = tp.references_to_symbol(body, symbol);
        expect_occurrences(t, &identifier_occurrences(t, sf, "e", &references), &[1, 2]);
        if references[0].parent().is_none_or(|p| p.kind() != Kind::ShorthandPropertyAssignment) {
            t.fatal("expected the first reference to be the shorthand property name");
        }
    });
}

// go test: internal/typeparser/references_to_symbol_test.go:84 TestReferencesToSymbol_ImportAlias 09ec0e08eb60578d
#[test]
fn test_references_to_symbol_import_alias() {
    run(|t| {
        t.parallel();

        let (mut compiled, source_files) = compile_and_get_checker_and_source_files_internal(
            t,
            &[
                ("/.src/a.ts", "export const Foo = 1"),
                (
                    "/.src/b.ts",
                    "import { Foo as Bar } from \"./a\"\nexport const value = Bar\nexport const object = { Bar }",
                ),
            ],
        );
        let mut tp = compiled.type_parser();

        let a = source_files["/.src/a.ts"];
        let b = source_files["/.src/b.ts"];
        let Some(module_symbol) = get_symbol_of_declaration(tp.checker, a.as_node()) else {
            t.fatal("expected module symbol for a.ts");
        };
        let Some(export_foo) =
            tp.checker.try_get_member_in_module_exports_and_properties("Foo", module_symbol)
        else {
            t.fatal("expected exported Foo symbol");
        };

        // Skip the import declaration, whose specifier names also resolve to Foo.
        let statements = b.statements().nodes();
        let mut occurrences = Vec::new();
        for &statement in &statements[1..] {
            let references = tp.references_to_symbol(statement, export_foo);
            occurrences.extend(identifier_occurrences(t, b, "Bar", &references));
        }
        expect_occurrences(t, &occurrences, &[1, 2]);
    });
}

// go test: internal/typeparser/references_to_symbol_test.go:115 TestReferencesToSymbol_MergedExport 853e91a018d68da5
#[test]
fn test_references_to_symbol_merged_export() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            "export const Foo = 1\nexport const Bar = Foo + 1",
        );
        let mut tp = compiled.type_parser();

        let Some(module_symbol) = get_symbol_of_declaration(tp.checker, sf.as_node()) else {
            t.fatal("expected module symbol");
        };
        let Some(export_foo) =
            tp.checker.try_get_member_in_module_exports_and_properties("Foo", module_symbol)
        else {
            t.fatal("expected exported Foo symbol");
        };
        let references = tp.references_to_symbol(sf.as_node(), export_foo);
        expect_occurrences(t, &identifier_occurrences(t, sf, "Foo", &references), &[0, 1]);
    });
}

// go test: internal/typeparser/references_to_symbol_test.go:133 TestReferencesToSymbol_ShadowedBindingIsNotAReference e91224122ac52671
#[test]
fn test_references_to_symbol_shadowed_binding_is_not_a_reference() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            "export function f(e: number) {\n  const g = (e: string) => ({ e, length: e.length })\n  return [e, g]\n}",
        );
        let mut tp = compiled.type_parser();

        let Some(symbol) = tp.get_symbol_at_location(find_identifier_by_text(t, sf, "e", 0)) else {
            t.fatal("expected parameter symbol");
        };
        let references = tp.references_to_symbol(sf.as_node(), symbol);
        expect_occurrences(t, &identifier_occurrences(t, sf, "e", &references), &[0, 4]);
    });
}

// go test: internal/typeparser/references_to_symbol_test.go:149 TestReferencesToSymbol_NilInputs 0f4f3829119a1d31
#[test]
fn test_references_to_symbol_nil_inputs() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_internal(t, "export const e = 1");
        let mut tp = compiled.type_parser();

        let Some(symbol) = tp.get_symbol_at_location(find_identifier_by_text(t, sf, "e", 0)) else {
            t.fatal("expected symbol");
        };
        let references = tp.references_to_symbol(None, symbol);
        if !references.is_empty() {
            t.fatal(format!("expected no references for a nil root, got {}", references.len()));
        }
        let references = tp.references_to_symbol(sf.as_node(), None);
        if !references.is_empty() {
            t.fatal(format!("expected no references for a nil symbol, got {}", references.len()));
        }
    });
}
