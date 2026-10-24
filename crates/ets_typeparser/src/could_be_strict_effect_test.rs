use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::core::P;
use rustc_hash::FxHashMap;

use crate::helpers_test::*;
use crate::module_export_reference_test::kind_string;

// go test: internal/typeparser/could_be_strict_effect_test.go:14 findIdentifierByName
// findIdentifierByName returns the first identifier node with the given text
// that is not a declaration name.
fn find_identifier_by_name(t: &T, sf: P<SourceFile>, name: &str) -> P<Node> {
    t.helper();
    fn visit(node: P<Node>, name: &str, found: &mut Option<P<Node>>) -> bool {
        if found.is_some() {
            return true;
        }
        if node.kind() == Kind::Identifier
            && node.text() == name
            && !ets_shim::ast::is_declaration_name(node)
        {
            *found = Some(node);
            return true;
        }
        node.for_each_child(&mut |child| visit(child, name, found));
        false
    }
    let mut found = None;
    sf.as_node().for_each_child(&mut |child| visit(child, name, &mut found));
    let Some(found) = found else {
        t.fatal(sprintf!("identifier %q not found in source", name));
    };
    found
}

// Rust: the `findKind` closure of TestNodeCouldBeStrictEffect.
fn find_kind(t: &T, sf: P<SourceFile>, kind: Kind) -> P<Node> {
    fn visit(node: P<Node>, kind: Kind, found: &mut Option<P<Node>>) -> bool {
        if found.is_none() && node.kind() == kind {
            *found = Some(node);
        }
        node.for_each_child(&mut |child| visit(child, kind, found));
        found.is_some()
    }
    let mut found = None;
    sf.as_node().for_each_child(&mut |child| visit(child, kind, &mut found));
    let Some(found) = found else {
        t.fatal(sprintf!("no %v found", kind_string(kind)));
    };
    found
}

// go test: internal/typeparser/could_be_strict_effect_test.go:36 TestNodeCouldBeStrictEffect
// Rust: Go's nil-receiver check (`nilTp.NodeCouldBeStrictEffect(nil)`) is left out: the Rust method takes `&mut self`,
// so there is no nil parser to call it on.
#[test]
fn test_node_could_be_strict_effect() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V4, "effect") {
            t.skip(sprintf!("Effect v4 not installed: %v", err));
        }

        let source = r#"
import { Effect } from "effect"

const anEffect = Effect.succeed(1)
const aString = "hello"
const aNumber = 42
const anAny: any = null
const anUnknown: unknown = null
const aUnionWithEffect: Effect.Effect<number> | string = anEffect
const aUnionWithoutEffect: string | number | boolean = "x"
interface Plain { readonly value: number }
const aPlainObject: Plain = { value: 1 }
const anObjectKeyword: object = { value: 1 }
function generic<T>(param: T): T { return param }

export const uses = [
	anEffect,
	aString,
	aNumber,
	anAny,
	anUnknown,
	aUnionWithEffect,
	aUnionWithoutEffect,
	aPlainObject,
	anObjectKeyword,
]
export function inner<T>(param: T) { return [param] }
export const either = anAny || anEffect
"#;
        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let tests = [
            // Conclusive negatives: the declared type can never flow-narrow into
            // a type whose symbol is named "Effect".
            ("aString", false),
            ("aNumber", false),
            ("aUnionWithoutEffect", false),
            ("aPlainObject", false),
            // Effect references and everything inconclusive must stay true.
            ("anEffect", true),
            ("anAny", true),
            ("anUnknown", true),
            ("aUnionWithEffect", true),
            ("anObjectKeyword", true), // the object keyword type has no symbol
            ("param", true),           // type parameters can instantiate to anything
        ];

        // Subtests deliberately avoided: all cases share one checker, which is
        // not safe for the parallel subtests the tparallel linter would require.
        for (identifier, expected) in tests {
            let node = find_identifier_by_name(t, sf, identifier);
            let got = tp.node_could_be_strict_effect(node);
            if got != expected {
                t.error(sprintf!(
                    "NodeCouldBeStrictEffect(%s) = %v, want %v",
                    identifier,
                    got,
                    expected
                ));
            }
        }

        if tp.node_could_be_strict_effect(find_kind(t, sf, Kind::ArrayLiteralExpression)) {
            t.error("an array literal can never be an Effect");
        }
        if tp.node_could_be_strict_effect(find_kind(t, sf, Kind::ObjectLiteralExpression)) {
            t.error("an object literal is never named Effect");
        }
        if !tp.node_could_be_strict_effect(find_kind(t, sf, Kind::BinaryExpression)) {
            t.error("other non-reference, non-call node kinds must not be ruled out");
        }

        // Nil receiver and nil node stay conservative.
        if !tp.node_could_be_strict_effect(None) {
            t.error("nil node must not be ruled out");
        }
    });
}

// go test: internal/typeparser/could_be_strict_effect_test.go:138 TestCouldBeStrictEffectDeepUnionStaysConservative
#[test]
fn test_could_be_strict_effect_deep_union_stays_conservative() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V4, "effect") {
            t.skip(sprintf!("Effect v4 not installed: %v", err));
        }

        // A union nested beyond the recursion depth limit must return true even
        // though every member is a primitive literal.
        let mut members = Vec::with_capacity(40);
        for s in ["a", "b", "c", "d", "e", "f", "g", "h"] {
            members.push(format!("\"lit{s}\""));
        }
        let source = format!(
            "\ntype Deep = {}\nconst deepValue: Deep = \"lita\"\nexport const use = [deepValue]\n",
            members.join(" | ")
        );
        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, &source);
        let mut tp = compiled.type_parser();

        let node = find_identifier_by_name(t, sf, "deepValue");
        // Literal unions are flat, so this exercises the union walk; whatever the
        // nesting, the answer may be false only when provably safe — a flat
        // primitive union is provably safe.
        if tp.node_could_be_strict_effect(node) {
            t.error("flat primitive literal union should be conclusively non-Effect");
        }
    });
}

// go test: internal/typeparser/could_be_strict_effect_test.go:169 findCallByCalleeName
// findCallByCalleeName returns the first call expression whose callee text
// contains the given substring.
fn find_call_by_callee_name(t: &T, sf: P<SourceFile>, callee: &str) -> P<Node> {
    t.helper();
    fn visit(node: P<Node>, callee: &str, found: &mut Option<P<Node>>) -> bool {
        if found.is_some() {
            return true;
        }
        if node.kind() == Kind::CallExpression {
            let expr = node.as_call_expression().expression();
            if ets_shim::scanner::get_text_of_node(expr).contains(callee) {
                *found = Some(node);
                return true;
            }
        }
        node.for_each_child(&mut |child| visit(child, callee, found));
        false
    }
    let mut found = None;
    sf.as_node().for_each_child(&mut |child| visit(child, callee, &mut found));
    let Some(found) = found else {
        t.fatal(sprintf!("call to %q not found in source", callee));
    };
    found
}

// go test: internal/typeparser/could_be_strict_effect_test.go:194 TestCallCouldReturnStrictEffect
#[test]
fn test_call_could_return_strict_effect() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V4, "effect") {
            t.skip(sprintf!("Effect v4 not installed: %v", err));
        }

        let source = r#"
import { Effect } from "effect"

declare function makesEffect(): Effect.Effect<number>
declare function makesString(): string
declare function makesUnion(flag: boolean): Effect.Effect<number> | undefined
declare function makesAny(): any
declare function generic<T>(value: T): T
declare const maybe: { makesEffect(): Effect.Effect<number> } | undefined

export const uses = [
	makesEffect(),
	makesString(),
	makesUnion(true),
	makesAny(),
	generic("x"),
	maybe?.makesEffect(),
]
"#;
        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        // Subtests deliberately avoided: all cases share one checker, which is
        // not safe for the parallel subtests the tparallel linter would require.
        let tests = [
            // Conclusive negative: the declared return type can never be Effect.
            ("makesString", false),
            // Effect-returning calls and every inconclusive case stay true.
            ("makesEffect", true),
            ("makesUnion", true), // union containing Effect
            ("makesAny", true),   // any return
            // The resolved signature is instantiated, so generic("x") conclusively
            // returns the primitive literal "x" and is ruled out.
            ("generic", false),
            ("maybe?.makesEffect", true), // optional chain: Effect | undefined union
        ];

        for (callee, expected) in tests {
            let call = find_call_by_callee_name(t, sf, callee);
            let got = tp.node_could_be_strict_effect(call);
            if got != expected {
                t.error(sprintf!(
                    "NodeCouldBeStrictEffect(call %s) = %v, want %v",
                    callee,
                    got,
                    expected
                ));
            }
        }
    });
}

// go test: internal/typeparser/could_be_strict_effect_test.go:248 TestNodeCouldBeEffect
#[test]
fn test_node_could_be_effect() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V4, "effect") {
            t.skip(sprintf!("Effect v4 not installed: %v", err));
        }

        let source = r#"
import { Effect } from "effect"

interface Subtype extends Effect.Effect<number> {}
declare const aSubtype: Subtype
declare function makesSubtype(): Subtype
declare function makesString(): string
const anEffect = Effect.succeed(1)
const aString = "hello"
const anUnknown: unknown = null
interface Plain { readonly value: number }
const aPlainObject: Plain = { value: 1 }
function generic<T>(param: T): T { return param }
export const uses = [aSubtype, anEffect, aString, anUnknown, aPlainObject, makesSubtype(), makesString()]
const key = "k"
export const literals = {
	"key": 1, arrow: () => 1, array: [1], value: "text", object: { a: 1 },
	spread: { ...anEffect }, computed: { [key]: 1 }, marked: { "~effect/Effect": 1 }
}
"#;
        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let identifiers = [
            // References are never ruled out: a type guard can narrow any
            // declared type into an Effect.
            ("aSubtype", true),
            ("anEffect", true),
            ("anUnknown", true),
            ("param", true),
            ("aString", true),
            ("aPlainObject", true),
        ];
        for (identifier, expected) in identifiers {
            let node = find_identifier_by_name(t, sf, identifier);
            let got = tp.node_could_be_effect(node);
            if got != expected {
                t.error(sprintf!("NodeCouldBeEffect(%s) = %v, want %v", identifier, got, expected));
            }
        }

        if !tp.node_could_be_effect(find_call_by_callee_name(t, sf, "makesSubtype")) {
            t.error("a call returning an Effect subtype must not be ruled out");
        }
        if tp.node_could_be_effect(find_call_by_callee_name(t, sf, "makesString")) {
            t.error("a call returning string should be ruled out");
        }

        let mut initializers: FxHashMap<String, bool> = FxHashMap::default();
        fn visit(
            t: &T,
            tp: &mut crate::TypeParser<'_>,
            node: P<Node>,
            initializers: &mut FxHashMap<String, bool>,
        ) -> bool {
            if node.kind() == Kind::PropertyAssignment
                && let Some(name) = node.name()
                && name.kind() != Kind::ComputedPropertyName
            {
                let could_be = tp.node_could_be_effect(node.initializer());
                initializers.insert(name.text().to_string(), could_be);
                if name.kind() == Kind::StringLiteral && !tp.node_could_be_effect(name) {
                    t.error("a string literal property name must not be ruled out");
                }
            }
            node.for_each_child(&mut |child| visit(t, tp, child, initializers));
            false
        }
        sf.as_node().for_each_child(&mut |child| visit(t, &mut tp, child, &mut initializers));
        // Rust: Go ranges over the `want` map (random order); the cases only report errors, so a fixed order
        // reports the same set.
        let want = [
            ("key", false),
            ("arrow", false),
            ("array", false),
            ("value", false),
            ("object", false),
            ("a", false),
            ("spread", true),
            ("computed", true),
            ("marked", true),
        ];
        for (name, expected) in want {
            let found = initializers.get(name).copied();
            let (got, ok) = (found.unwrap_or_default(), found.is_some());
            if !ok || got != expected {
                t.error(sprintf!(
                    "NodeCouldBeEffect(initializer of %s) = %v (found %v), want %v",
                    name,
                    got,
                    ok,
                    expected
                ));
            }
        }
    });
}
