use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::checker::Type;
use ets_shim::core::P;
use ets_shim::core::context::Context;
use rustc_hash::FxHashMap;

use crate::effect_context_test::{EFFECT_CONTEXT_FIXTURE_JS, EFFECT_CONTEXT_FIXTURE_TS};
use crate::get_type_at_location::{
    is_inside_type_only_heritage_expression, is_type_at_location_candidate,
};
use crate::helpers_test::*;
use crate::module_export_reference_test::{compile_fixtures_with_js, kind_string, node_pos};
use crate::*;

// go test: internal/typeparser/get_type_at_location_test.go:12 typeAtLocationGuardFixtureMain 6313ba02ffc2e50d
const TYPE_AT_LOCATION_GUARD_FIXTURE_MAIN: &str = r#"import { value } from "./dep.js"
import * as ns from "./dep.js"
import type { Shape } from "./dep.js"
import d, { type Shape as Alias } from "./dep.js"
export function run(n: number = value, shape?: Shape & Alias): number {
  label: if (n > 0) { for (const item of [n, ns.value, d]) { n += item; break label } }
  switch (n) { case 1: return 1; default: }
  try { return shape?.n ?? n } catch (error) { throw error } finally { n = 0 }
}
export default run
"#;

// go test: internal/typeparser/get_type_at_location_test.go:24 typeAtLocationGuardFixtureDep cc6eb3731f5fa73f
const TYPE_AT_LOCATION_GUARD_FIXTURE_DEP: &str = r"export const value = 1
export interface Shape { n: number }
export default value
";

// go test: internal/typeparser/get_type_at_location_test.go:29 typeAtLocationGuardFixtureSyntax 3cf41512cdcf383f
const TYPE_AT_LOCATION_GUARD_FIXTURE_SYNTAX: &str = concat!(
    "export {}\n",
    "namespace N { export interface Base { n: number } export class Value { n = 1 } }\n",
    "interface Derived extends N.Base {}\n",
    "class Child extends N.Value implements N.Base, N.Base {}\n",
    "declare function tag(parts: TemplateStringsArray, ...values: symbol[]): string\n",
    "declare const s: symbol\n",
    "const tagged = tag`value ${s} and ${s}`\n",
    "const text = `value ${1}`\n",
    "const deferred = import.defer(\"./dep.js\")\n",
    "function target() { return new.target }\n",
    "const meta = import.meta\n",
    "type Mapped = { [K in \"a\"]: K }\n",
    "enum E { A = 1 }\n",
);

// go test: internal/typeparser/get_type_at_location_test.go:43 typeAtLocationGuardFixtureView 25390a444cd29c59
const TYPE_AT_LOCATION_GUARD_FIXTURE_VIEW: &str = r#"declare global { namespace JSX { interface IntrinsicElements { div: { title?: string } } } }
declare function View(props: { title: string }): any
export const view = <div title="value"><View title="child" />{"text"}</div>
export const fragment = <><View title={String(1)} /></>
"#;

// go test: internal/typeparser/get_type_at_location_test.go:49 typeAtLocationGuardFixtureJS 6091d5dcbf496f86
const TYPE_AT_LOCATION_GUARD_FIXTURE_JS: &str = r#"// @ts-check
import { value } from "./dep.js"
/** @typedef {{ n: number }} Local */
/** @param {Local} local @returns {number} */
export function read(local) { return local.n + value }
export const cast = /** @type {Local} */ ({ n: value })
"#;

// go test: internal/typeparser/get_type_at_location_test.go:59 getTypeAtLocationUnhoisted 96aab3849c23de10
// getTypeAtLocationUnhoisted is GetTypeAtLocation with the guards inside the
// cached computation, as before they were hoisted.
//
// Rust: Go's `CachedNode` over a store of the test's own: the links' `cached_node` selects a store of the links.
fn get_type_at_location_unhoisted(
    tp: &mut TypeParser<'_>,
    store: &mut DenseNodeStore<Option<P<Type>>>,
    node: P<Node>,
) -> Option<P<Type>> {
    if let Some(&value) = store.get(&node) {
        return value;
    }
    let value = if is_type_at_location_candidate(node) {
        tp.get_type_at_location_uncached(node)
    } else {
        None
    };
    store.insert(node, value);
    value
}

// go test: internal/typeparser/get_type_at_location_test.go:68 TestGetTypeAtLocationHoistedGuardsMatchUnhoisted d404f89e9d3513cf
#[test]
fn test_get_type_at_location_hoisted_guards_match_unhoisted() {
    run(|t| {
        t.parallel();

        let cases: [(&str, Option<EffectVersion>, Vec<(&str, &str)>); 2] = [
            (
                "syntax",
                None,
                vec![
                    ("/.src/main.ts", TYPE_AT_LOCATION_GUARD_FIXTURE_MAIN),
                    ("/.src/dep.ts", TYPE_AT_LOCATION_GUARD_FIXTURE_DEP),
                    ("/.src/syntax.ts", TYPE_AT_LOCATION_GUARD_FIXTURE_SYNTAX),
                    ("/.src/view.tsx", TYPE_AT_LOCATION_GUARD_FIXTURE_VIEW),
                    ("/.src/read.js", TYPE_AT_LOCATION_GUARD_FIXTURE_JS),
                ],
            ),
            (
                "effect",
                Some(EFFECT_V4),
                vec![
                    ("/.src/fixture.ts", EFFECT_CONTEXT_FIXTURE_TS),
                    ("/.src/fixture.js", EFFECT_CONTEXT_FIXTURE_JS),
                ],
            ),
        ];
        for (name, version, sources) in cases {
            t.run(name, |t| {
                t.parallel();
                let (mut compiled, source_files) = compile_fixtures_with_js(t, version, &sources);
                let mut tp = compiled.type_parser();

                struct State {
                    unhoisted: DenseNodeStore<Option<P<Type>>>,
                    nodes: i64,
                    typed: i64,
                    rejected: i64,
                    guards: FxHashMap<&'static str, i64>,
                }
                let mut state = State {
                    unhoisted: DenseNodeStore::default(),
                    nodes: 0,
                    typed: 0,
                    rejected: 0,
                    guards: FxHashMap::default(),
                };
                for (i, &sf) in source_files.iter().enumerate() {
                    // The first file is compared before any file is checked. Asked
                    // about an unchecked file, the checker can build an equal but new
                    // type on each evaluation, so there the types are compared printed.
                    let checked = i > 0;
                    if i == 1 {
                        for &other in &source_files {
                            tp.checker().get_diagnostics_exported(&Context::background(), other);
                        }
                    }
                    fn same(
                        tp: &mut TypeParser<'_>,
                        checked: bool,
                        a: Option<P<Type>>,
                        b: Option<P<Type>>,
                    ) -> bool {
                        if a == b {
                            return true;
                        }
                        match (a, b) {
                            (Some(a), Some(b)) if !checked => {
                                type_string(tp, a) == type_string(tp, b)
                            }
                            _ => false,
                        }
                    }
                    fn print(tp: &mut TypeParser<'_>, t: Option<P<Type>>) -> String {
                        t.map_or_else(|| "<nil>".to_string(), |t| type_string(tp, t))
                    }
                    fn visit(
                        t: &T,
                        tp: &mut TypeParser<'_>,
                        state: &mut State,
                        i: usize,
                        checked: bool,
                        sf: P<SourceFile>,
                        node: P<Node>,
                    ) -> bool {
                        state.nodes += 1;
                        // even files ask the unhoisted ordering first, odd files the hoisted one
                        let (first, second) = if i.is_multiple_of(2) {
                            let first =
                                get_type_at_location_unhoisted(tp, &mut state.unhoisted, node);
                            (first, tp.get_type_at_location(node))
                        } else {
                            let first = tp.get_type_at_location(node);
                            (first, get_type_at_location_unhoisted(tp, &mut state.unhoisted, node))
                        };
                        if !same(tp, checked, first, second) {
                            t.error(ets_go::sprintf!(
                                "%s: %s at %d: %s, then %s",
                                sf.file_name(),
                                kind_string(node.kind()),
                                node_pos(node),
                                print(tp, first),
                                print(tp, second)
                            ));
                        }
                        // repeated queries answer from the caches
                        let again = tp.get_type_at_location(node);
                        if again != tp.get_type_at_location(node)
                            || !same(tp, checked, again, first)
                        {
                            t.error(ets_go::sprintf!(
                                "%s: %s at %d: repeated query got %s, want %s",
                                sf.file_name(),
                                kind_string(node.kind()),
                                node_pos(node),
                                print(tp, again),
                                print(tp, first)
                            ));
                        }
                        if first.is_some() {
                            state.typed += 1;
                        }
                        if !is_type_at_location_candidate(node) {
                            state.rejected += 1;
                            let parent = node.parent();
                            for (guard, hit) in [
                                (
                                    "jsx tag name",
                                    parent.is_some() && ets_shim::ast::is_jsx_tag_name(node),
                                ),
                                (
                                    "jsx attribute name",
                                    parent.is_some_and(|parent| {
                                        ets_shim::ast::is_jsx_attribute(parent)
                                            && parent.name() == Some(node)
                                    }),
                                ),
                                ("import clause", node.kind() == Kind::ImportClause),
                                ("tagged template", node.kind() == Kind::TemplateExpression),
                                ("meta property call", node.kind() == Kind::MetaProperty),
                                (
                                    "type-only heritage",
                                    is_inside_type_only_heritage_expression(node),
                                ),
                            ] {
                                if hit {
                                    *state.guards.entry(guard).or_default() += 1;
                                }
                            }
                            if tp.links().type_at_location.contains_key(&node) {
                                t.error(ets_go::sprintf!(
                                    "%s: %s at %d: rejected node has a cache entry",
                                    sf.file_name(),
                                    kind_string(node.kind()),
                                    node_pos(node)
                                ));
                            }
                        } else if !tp.links().type_at_location.contains_key(&node) {
                            t.error(ets_go::sprintf!(
                                "%s: %s at %d: admitted node has no cache entry",
                                sf.file_name(),
                                kind_string(node.kind()),
                                node_pos(node)
                            ));
                        }
                        ets_shim::ast::for_each_child_and_jsdoc(node, sf.get(), &mut |child| {
                            visit(t, tp, state, i, checked, sf, child)
                        });
                        false
                    }
                    visit(t, &mut tp, &mut state, i, checked, sf, sf.as_node());
                }
                t.log(ets_go::sprintf!(
                    "%d files, %d nodes, %d typed, %d rejected",
                    source_files.len() as i64,
                    state.nodes,
                    state.typed,
                    state.rejected
                ));
                if state.typed == 0 || state.rejected == 0 {
                    t.fatal("fixtures do not exercise the guards");
                }
                // Type-only heritage elements parse as type references with some
                // providers, so that guard is not required to fire.
                if version.is_none() {
                    for guard in [
                        "jsx tag name",
                        "jsx attribute name",
                        "import clause",
                        "tagged template",
                        "meta property call",
                    ] {
                        if state.guards.get(guard).copied().unwrap_or_default() == 0 {
                            t.error(ets_go::sprintf!(
                                "fixtures do not exercise the %s guard",
                                guard
                            ));
                        }
                    }
                }
            });
        }
    });
}

// go test: internal/typeparser/get_type_at_location_test.go:179 TestGetTypeAtLocationImportClause 4f7612cfc4035ae1
// Import clauses without a default binding declare no symbol; before the
// KindImportClause guard, checker.GetTypeAtLocation panicked on them with a
// nil symbol dereference (recovered silently, see issue #301 investigation).
#[test]
fn test_get_type_at_location_import_clause() {
    run(|t| {
        t.parallel();

        let sources = [
            (
                "/.src/dep.ts",
                "
export const A = 1
export type T = string
const d = { A }
export default d
",
            ),
            (
                "/.src/main.ts",
                "
import { A } from \"./dep.js\"
import * as ns from \"./dep.js\"
import type { T } from \"./dep.js\"
import d from \"./dep.js\"
export const use: T = String(A + ns.A + d.A)
",
            ),
        ];

        let (mut compiled, source_files) =
            compile_and_get_checker_and_source_files_internal(t, &sources);
        let mut tp = compiled.type_parser();

        let mut clauses: Vec<P<Node>> = Vec::new();
        fn visit(node: P<Node>, clauses: &mut Vec<P<Node>>) -> bool {
            if node.kind() == Kind::ImportClause {
                clauses.push(node);
            }
            node.for_each_child(&mut |child| visit(child, clauses));
            false
        }
        source_files["/.src/main.ts"]
            .as_node()
            .for_each_child(&mut |child| visit(child, &mut clauses));

        if clauses.len() != 4 {
            t.fatal(ets_go::sprintf!("expected 4 import clauses, got %d", clauses.len() as i64));
        }

        for clause in clauses {
            if tp.get_type_at_location(clause).is_some() {
                let parent = clause.parent().expect("import clause parent");
                t.error(ets_go::sprintf!(
                    "expected nil type for import clause %q, got a type",
                    ets_shim::scanner::get_text_of_node(parent)
                ));
            }
        }
    });
}

// go test: internal/typeparser/get_type_at_location_test.go:227 checkedTypeSource 50f716d15785f4cd
// Re-evaluating these expressions after the check gives a different type than
// the check itself computed: the iterator's return literals were checked while
// the enclosing generic call was still resolving, and the narrowed reference
// was checked with its constraint substituted.
const CHECKED_TYPE_SOURCE: &str = r#"
declare const dual: {
  <DataLast extends (...args: Array<any>) => any, DataFirst extends (...args: Array<any>) => any>(
    arity: Parameters<DataFirst>["length"],
    body: DataFirst
  ): DataLast & DataFirst
}
export const takeWhileDefined: {
  <A>(f: (input: A) => A | undefined): (self: Iterable<A>) => Iterable<A>
  <A>(self: Iterable<A>, f: (input: A) => A | undefined): Iterable<A>
} = dual(2, <A>(self: Iterable<A>, f: (input: A) => A | undefined) => ({
  [Symbol.iterator]() {
    const iterator = self[Symbol.iterator]()
    return {
      next() {
        const result = iterator.next()
        if (result.done) {
          return { done: true, value: undefined }
        }
        return { done: false, value: f(result.value) }
      }
    }
  }
}))

export const getRandomValues = <T extends ArrayBufferView | null>(array: T): T => {
  if (array instanceof Uint8Array) {
    array[0] = 1
  }
  return array
}
"#;

// go test: internal/typeparser/get_type_at_location_test.go:260 TestGetTypeAtLocationReturnsCheckedType 3dfcaa87e84a60b9
#[test]
fn test_get_type_at_location_returns_checked_type() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, CHECKED_TYPE_SOURCE);
        compiled.checker.get_diagnostics_exported(&Context::background(), sf);
        let mut tp = compiled.type_parser();

        let mut got: FxHashMap<String, String> = FxHashMap::default();
        fn visit(
            tp: &mut TypeParser<'_>,
            sf: P<SourceFile>,
            got: &mut FxHashMap<String, String>,
            node: P<Node>,
        ) -> bool {
            if ets_shim::ast::is_return_statement(node)
                && let Some(expression) = node.expression()
                && ets_shim::ast::is_object_literal_expression(expression)
            {
                // Rust: Go's TypeToString of a nil type is the empty string here.
                let t = tp.get_type_at_location(expression);
                let text = t.map(|t| type_string(tp, t)).unwrap_or_default();
                got.insert(format!("return {}", scanner_text(sf, expression)), text);
            } else if ets_shim::ast::is_binary_expression(node)
                && node.as_binary_expression().operator_token().kind() == Kind::InstanceOfKeyword
            {
                let left = node.as_binary_expression().left();
                let t = tp.get_type_at_location(left);
                let text = t.map(|t| type_string(tp, t)).unwrap_or_default();
                got.insert(format!("instanceof {}", scanner_text(sf, left)), text);
            }
            node.for_each_child(&mut |child| visit(tp, sf, got, child));
            false
        }
        sf.as_node().for_each_child(&mut |child| visit(&mut tp, sf, &mut got, child));

        // Rust: Go ranges over a map literal (random order); the keys are checked in a fixed order.
        let expected = [
            ("return { done: true, value: undefined }", "{ done: boolean; value: undefined; }"),
            (
                "return { done: false, value: f(result.value) }",
                "{ done: boolean; value: A | undefined; }",
            ),
            ("instanceof array", "ArrayBufferView<ArrayBufferLike> | null"),
        ];
        for (key, want) in expected {
            let got = got.get(key).map(String::as_str).unwrap_or_default();
            if got != want {
                t.error(ets_go::sprintf!("%s: got %q, want %q", key, got, want));
            }
        }
    });
}

// go test: internal/typeparser/get_type_at_location_test.go:294 TestGetTypeAtLocationAddsNoDiagnosticsAfterCheck 96bcb272df48650a
#[test]
fn test_get_type_at_location_adds_no_diagnostics_after_check() {
    run(|t| {
        t.parallel();

        let source = CHECKED_TYPE_SOURCE.to_string()
            + "
declare function takesString<T extends string>(x: T): T
export const bad = takesString(1)
export function inferred() { return takesString(2) }
";
        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, &source);
        let mut tp = compiled.type_parser();
        let before = tp.checker().get_diagnostics_exported(&Context::background(), sf).len();
        if before == 0 {
            t.fatal("expected the source to have type errors");
        }

        fn visit(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
            if ets_shim::ast::is_expression_node(node) {
                tp.get_type_at_location(node);
            }
            node.for_each_child(&mut |child| visit(tp, child));
            false
        }
        sf.as_node().for_each_child(&mut |child| visit(&mut tp, child));

        let after = tp.checker().get_diagnostics_exported(&Context::background(), sf).len();
        if after != before {
            t.error(ets_go::sprintf!(
                "querying every expression added %d diagnostics",
                after as i64 - before as i64
            ));
        }
    });
}

// go test: internal/typeparser/get_type_at_location_test.go:323 scannerText 900bc4c580366402
fn scanner_text(sf: P<SourceFile>, node: P<Node>) -> &'static str {
    let pos = node.pos().source_offset().expect("a parsed node") as usize;
    let end = node.end().source_offset().expect("a parsed node") as usize;
    let mut text = &sf.text()[pos..end];
    while let Some(rest) = text.strip_prefix(' ').or_else(|| text.strip_prefix('\n')) {
        text = rest;
    }
    text
}
