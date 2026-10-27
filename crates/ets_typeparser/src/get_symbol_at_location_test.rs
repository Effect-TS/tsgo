// Ported from effect-tsgo's internal/typeparser/get_symbol_at_location_test.go (see helpers_test.rs).

use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::core::P;
use ets_shim::core::context::Context;

use crate::helpers_test::*;

// go test: internal/typeparser/get_symbol_at_location_test.go:10 findDeleteExpression de70185f18d23f11
fn find_delete_expression(t: &T, sf: P<SourceFile>) -> P<Node> {
    t.helper();
    let mut found = None;
    fn visit(node: P<Node>, found: &mut Option<P<Node>>) -> bool {
        if node.kind() == Kind::DeleteExpression {
            *found = Some(node);
        }
        node.for_each_child(&mut |child| visit(child, found));
        false
    }
    sf.as_node().for_each_child(&mut |child| visit(child, &mut found));
    let Some(found) = found else {
        t.fatal("delete expression not found");
    };
    found
}

/// Rust-only: the codes of `c.GetDiagnostics(ctx, sf)`.
fn diagnostic_codes(compiled: &mut Compiled, sf: P<SourceFile>) -> Vec<i32> {
    let ctx = Context::background();
    compiled.checker.get_diagnostics_exported(&ctx, sf).iter().map(|d| d.code()).collect()
}

// go test: internal/typeparser/get_symbol_at_location_test.go:28 TestGetSymbolAtLocationDeleteIndexDiagnostics 36376d5a76f9567e
#[test]
fn test_get_symbol_at_location_delete_index_diagnostics() {
    run(|t| {
        t.parallel();
        for (name, expression, property) in [
            ("plain", "map.x", true),
            ("parenthesized", "(map.x)", true),
            ("multiply_parenthesized", "(((map.x)))", true),
            ("optional", "map?.x", true),
            ("optional_parenthesized", "(map?.x)", true),
            ("optional_multiply_parenthesized", "(((map?.x)))", true),
            ("element", "map[\"x\"]", false),
            ("element_parenthesized", "(map[\"x\"])", false),
            ("element_optional", "map?.[\"x\"]", false),
        ] {
            let mut queries = vec!["access"];
            if property {
                queries.push("name");
            }
            for query in queries {
                t.run(&format!("{name}/{query}"), |t| {
                    t.parallel();
                    let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
                        t,
                        &format!(
                            "declare const map: {{ [key: string]: number }}; delete {expression}; export {{}};"
                        ),
                    );
                    let diagnostics = diagnostic_codes(&mut compiled, sf);
                    if !diagnostics.is_empty() {
                        t.fatal(ets_go::sprintf!(
                            "expected no initial diagnostics, got %v",
                            format!("{diagnostics:?}")
                        ));
                    }
                    let delete_expression = find_delete_expression(t, sf);
                    let mut location =
                        ets_shim::ast::skip_parentheses(delete_expression.expression().expect("operand"));
                    if query == "name" {
                        location = location.name().expect("property name");
                    }
                    let mut tp = compiled.type_parser();
                    tp.get_symbol_at_location(location);
                    let typ = tp.get_type_at_location(delete_expression);
                    if typ.is_none_or(|typ| type_string(&mut tp, typ) != "boolean") {
                        t.fatal("expected delete type boolean");
                    }
                    drop(tp);
                    for code in diagnostic_codes(&mut compiled, sf) {
                        t.error(ets_go::sprintf!(
                            "unexpected diagnostic after symbol and delete type lookup: TS%d",
                            code
                        ));
                    }
                });
            }
        }
    });
}

// go test: internal/typeparser/get_symbol_at_location_test.go:77 TestGetSymbolAtLocationDeletePreservesOtherSymbols f3fea3234ab0fc84
#[test]
fn test_get_symbol_at_location_delete_preserves_other_symbols() {
    run(|t| {
        t.parallel();
        for (name, source, identifier, occurrence, parent, literal, want_symbol) in [
            (
                "receiver",
                "declare const map: { [key: string]: number }; delete map.x;",
                "map",
                1,
                false,
                false,
                "map",
            ),
            (
                "parenthesized_receiver",
                "declare const map: { [key: string]: number }; delete (((map.x)));",
                "map",
                1,
                false,
                false,
                "map",
            ),
            (
                "optional_receiver",
                "declare const map: { [key: string]: number }; delete map?.x;",
                "map",
                1,
                false,
                false,
                "map",
            ),
            (
                "nested_receiver_name",
                "declare const root: { inner: { [key: string]: number } }; delete root.inner.x;",
                "inner",
                1,
                false,
                false,
                "inner",
            ),
            (
                "nested_receiver_access",
                "declare const root: { inner: { [key: string]: number } }; delete root.inner.x;",
                "inner",
                1,
                true,
                false,
                "inner",
            ),
            (
                "computed_key_name",
                "declare const map: { [key: string]: number }; declare const keys: { current: string }; delete map[keys.current];",
                "current",
                1,
                false,
                false,
                "current",
            ),
            (
                "computed_key_access",
                "declare const map: { [key: string]: number }; declare const keys: { current: string }; delete map[keys.current];",
                "current",
                1,
                true,
                false,
                "current",
            ),
            (
                "optional_computed_key_access",
                "declare const map: { [key: string]: number }; declare const keys: { current: string }; delete map?.[keys.current];",
                "current",
                1,
                true,
                false,
                "current",
            ),
            (
                "literal_key",
                "declare const map: { x?: number }; delete map[\"x\"];",
                "",
                0,
                false,
                true,
                "x",
            ),
            (
                "optional_literal_key",
                "declare const map: { x?: number }; delete map?.[\"x\"];",
                "",
                0,
                false,
                true,
                "x",
            ),
            (
                "non_delete_name",
                "declare const map: { x: number; y?: number }; map.x; delete map.y;",
                "x",
                1,
                false,
                false,
                "x",
            ),
            (
                "non_delete_access",
                "declare const map: { x: number; y?: number }; map.x; delete map.y;",
                "x",
                1,
                true,
                false,
                "x",
            ),
        ] {
            t.run(name, |t| {
                t.parallel();
                let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
                    t,
                    &format!("{source} export {{}};"),
                );
                let diagnostics = diagnostic_codes(&mut compiled, sf);
                if !diagnostics.is_empty() {
                    t.fatal(ets_go::sprintf!(
                        "expected no initial diagnostics, got %v",
                        format!("{diagnostics:?}")
                    ));
                }
                let delete_expression = find_delete_expression(t, sf);
                let location = if literal {
                    delete_expression
                        .expression()
                        .expect("operand")
                        .as_element_access_expression()
                        .argument_expression
                } else {
                    let mut location = find_identifier_by_text(t, sf, identifier, occurrence);
                    if parent {
                        location = location.parent().expect("parent");
                    }
                    location
                };
                let mut tp = compiled.type_parser();
                let symbol = tp.get_symbol_at_location(location);
                if symbol.is_none_or(|symbol| symbol.name() != want_symbol) {
                    t.fatal(ets_go::sprintf!(
                        "expected symbol %q, got %v",
                        want_symbol,
                        symbol.map_or("<nil>", |symbol| symbol.name())
                    ));
                }
                let typ = tp.get_type_at_location(delete_expression);
                if typ.is_none_or(|typ| type_string(&mut tp, typ) != "boolean") {
                    t.fatal("expected delete type boolean");
                }
                drop(tp);
                let diagnostics = diagnostic_codes(&mut compiled, sf);
                if !diagnostics.is_empty() {
                    t.fatal(ets_go::sprintf!(
                        "expected no diagnostics after symbol and delete type lookup, got %v",
                        format!("{diagnostics:?}")
                    ));
                }
            });
        }
    });
}

// go test: internal/typeparser/get_symbol_at_location_test.go:131 TestGetSymbolAtLocationDeleteRequiredPropertyDiagnostics 94ff4a56e9353f35
#[test]
fn test_get_symbol_at_location_delete_required_property_diagnostics() {
    run(|t| {
        t.parallel();
        for query in ["access", "name"] {
            t.run(query, |t| {
                t.parallel();
                let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
                    t,
                    "declare const map: { x: number }; delete map.x; export {};",
                );
                let diagnostics = diagnostic_codes(&mut compiled, sf);
                if diagnostics.len() != 1 || diagnostics[0] != 2790 {
                    t.fatal(ets_go::sprintf!(
                        "expected initial TS2790, got %v",
                        format!("{diagnostics:?}")
                    ));
                }
                let delete_expression = find_delete_expression(t, sf);
                let mut location = delete_expression.expression().expect("operand");
                if query == "name" {
                    location = location.name().expect("property name");
                }
                let mut tp = compiled.type_parser();
                tp.get_symbol_at_location(location);
                let typ = tp.get_type_at_location(delete_expression);
                if typ.is_none_or(|typ| type_string(&mut tp, typ) != "boolean") {
                    t.fatal("expected delete type boolean");
                }
                drop(tp);
                let diagnostics = diagnostic_codes(&mut compiled, sf);
                if diagnostics.is_empty() {
                    t.fatal("expected TS2790 after symbol and delete type lookup, got none");
                }
                for code in diagnostics {
                    if code != 2790 {
                        t.error(ets_go::sprintf!(
                            "unexpected diagnostic after symbol and delete type lookup: TS%d",
                            code
                        ));
                    }
                }
            });
        }
    });
}
