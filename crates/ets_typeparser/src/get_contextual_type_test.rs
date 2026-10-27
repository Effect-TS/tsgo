use std::sync::Arc;

use ets_go::sprintf;
use ets_shim::checker::ContextFlags;
use ets_shim::core::context::Context;

use crate::helpers_test::*;

// go test: internal/typeparser/get_contextual_type_test.go:11 TestGetContextualTypeInGeneratorExpressions 549c472da85d2807
#[test]
fn test_get_contextual_type_in_generator_expressions() {
    run(|t| {
        t.parallel();

        let literal = "const options: Options = { value: 42 };";
        for (name, source, skip) in [
            ("inferred generator", format!("const gen = function* () {{ {literal} }};"), true),
            ("named generator", format!("const gen = function* named() {{ {literal} }};"), true),
            ("async generator", format!("const gen = async function* () {{ {literal} }};"), true),
            (
                "contextually typed generator",
                format!(
                    "const gen: () => Generator<never, void, unknown> = function* () {{ {literal} }};"
                ),
                true,
            ),
            (
                "parameter default",
                format!(
                    "const gen = function* (arg = (() => {{ {literal} return options; }})()) {{}};"
                ),
                true,
            ),
            (
                "nested function",
                format!("const gen = function* () {{ function inner() {{ {literal} }} }};"),
                true,
            ),
            (
                "nested arrow",
                format!("const gen = function* () {{ const inner = () => {{ {literal} }}; }};"),
                true,
            ),
            (
                "annotated inner generator",
                format!(
                    "const gen = function* () {{ const inner = function* (): Generator<never, void, unknown> {{ {literal} }}; }};"
                ),
                true,
            ),
            (
                "annotated generator",
                format!(
                    "const gen = function* (): Generator<never, void, unknown> {{ {literal} }};"
                ),
                false,
            ),
            ("generator declaration", format!("function* gen() {{ {literal} }}"), false),
            ("generator method", format!("class C {{ *gen() {{ {literal} }} }}"), false),
            ("ordinary function", format!("const gen = function () {{ {literal} }};"), false),
            (
                "sibling generator",
                format!(
                    "declare function accept(a: Options, b: () => Generator<never, void, unknown>): void; accept((() => {{ {literal} return options; }})(), function* () {{}});"
                ),
                false,
            ),
        ] {
            t.run(name, |t| {
                t.parallel();
                let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
                    t,
                    &format!("interface Options {{ value: number }}; {source}"),
                );
                let mut tp = compiled.type_parser();
                let node = find_identifier_by_text(t, sf, "options", 0)
                    .parent()
                    .and_then(|parent| parent.initializer())
                    .expect("options initializer");
                for flags in [
                    ContextFlags::None,
                    ContextFlags::NoConstraints,
                    ContextFlags::IgnoreNodeInferences,
                    ContextFlags::IgnoreNodeInferences | ContextFlags::NoConstraints,
                ] {
                    let got = tp.get_contextual_type(node, flags);
                    if skip && flags.intersects(ContextFlags::IgnoreNodeInferences) {
                        if let Some(got) = got {
                            t.error(ets_go::sprintf!(
                                "flags %d: got %s, want unavailable contextual evidence",
                                flags.bits(),
                                type_string(&mut tp, got)
                            ));
                        }
                    } else if let Some(got) = got {
                        let name = type_string(&mut tp, got);
                        if name != "Options" {
                            t.error(ets_go::sprintf!(
                                "flags %d: contextual type = %q, want Options",
                                flags.bits(),
                                name
                            ));
                        }
                    } else {
                        t.error(ets_go::sprintf!(
                            "flags %d: contextual type is nil, want Options",
                            flags.bits()
                        ));
                    }
                }
            });
        }
    });
}

// go test: internal/typeparser/get_contextual_type_test.go:55 TestGetContextualTypeOnGeneratorExpression 46fe8ffed4e4a403
#[test]
fn test_get_contextual_type_on_generator_expression() {
    run(|t| {
        t.parallel();
        let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
            t,
            "declare function accept(body: () => Generator<number, void, unknown>): void;
accept(function* () { yield 1; });",
        );
        let mut tp = compiled.type_parser();
        let node = sf.as_node().statements()[1].expression().expect("call").arguments()[0];
        let got = tp.get_contextual_type(node, ContextFlags::None);
        if got
            .is_none_or(|got| type_string(&mut tp, got) != "() => Generator<number, void, unknown>")
        {
            t.fatal("normal generator context is not () => Generator<number, void, unknown>");
        }
        if let Some(got) = tp.get_contextual_type(node, ContextFlags::IgnoreNodeInferences) {
            t.fatal(ets_go::sprintf!(
                "ignored-inference generator context = %s, want nil",
                type_string(&mut tp, got)
            ));
        }
    });
}

// go test: internal/typeparser/get_contextual_type_test.go:69 TestInferredGeneratorContextualCaches 74b043dbe9b97134
// Rust: Go's second parser over the checker (`NewTypeParser(c.Program(), c)`) is made after the first is dropped
// (each borrows the checker); both use the checker's links, which the test compares by identity.
#[test]
fn test_inferred_generator_contextual_caches() {
    run(|t| {
        t.parallel();
        for constraint_first in [false, true] {
            t.run(&sprintf!("constraintFirst=%t", constraint_first), |t| {
                t.parallel();
                let (mut compiled, sf) = compile_and_get_checker_and_source_file_internal(
                    t,
                    "interface Options { value: number }
const gen = function* () { const options: Options = { value: 42 }; };",
                );
                let node = find_identifier_by_text(t, sf, "options", 0)
                    .parent()
                    .and_then(|parent| parent.initializer())
                    .expect("the options initializer");
                let mut tp = compiled.type_parser();
                if constraint_first
                    && let Some(got) = tp.cached_object_literal_constraint_contextual_type(node)
                {
                    t.fatal(sprintf!(
                        "constraint context = %s, want nil",
                        type_string(&mut tp, got)
                    ));
                }
                let got = tp.cached_object_literal_contextual_type(node);
                if got.is_none_or(|got| type_string(&mut tp, got) != "Options") {
                    t.fatal(sprintf!(
                        "normal context = %v, want Options",
                        got.map(|got| type_string(&mut tp, got)).unwrap_or_default()
                    ));
                }
                if let Some(got) = tp.cached_object_literal_constraint_contextual_type(node) {
                    t.fatal(sprintf!(
                        "constraint context = %s, want nil",
                        type_string(&mut tp, got)
                    ));
                }
                if tp.links().object_literal_constraint_contextual_type.get(&node) != Some(&None) {
                    t.fatal("skipped constraint lookup must cache a present nil");
                }
                let links = Arc::clone(&tp.links);
                drop(tp);
                let _ = compiled.checker.get_diagnostics_exported(&Context::background(), sf);
                let mut other = compiled.type_parser();
                if !Arc::ptr_eq(&other.links, &links)
                    || other.links().object_literal_constraint_contextual_type.get(&node)
                        != Some(&None)
                {
                    t.fatal("parsers over one checker must share the negative cache entry");
                }
                let got = other.cached_object_literal_contextual_type(node);
                if got.is_none_or(|got| type_string(&mut other, got) != "Options") {
                    t.fatal(sprintf!(
                        "shared normal context = %v, want Options",
                        got.map(|got| type_string(&mut other, got)).unwrap_or_default()
                    ));
                }
                if let Some(got) = other.cached_object_literal_constraint_contextual_type(node) {
                    t.fatal(sprintf!(
                        "shared constraint context after checking = %s, want nil",
                        type_string(&mut other, got)
                    ));
                }
            });
        }
    });
}
