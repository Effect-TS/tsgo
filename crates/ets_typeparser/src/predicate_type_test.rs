// Ported from effect-tsgo's internal/typeparser/predicate_type_test.go (see helpers_test.rs).

use ets_go::sprintf;
use ets_shim::ast::Node;
use ets_shim::core::P;

use crate::helpers_test::*;
use crate::returning_dispatch_test::{
    find_returning_dispatch_test_function, returning_dispatch_node_text,
};
use crate::*;

// go test: internal/typeparser/predicate_type_test.go:12 predicateTestImports 255894c6f7af5c15
const PREDICATE_TEST_IMPORTS: &str = r#"
	import { Predicate } from "effect"
	import * as P from "effect/Predicate"
	import { isTagged as tagged } from "effect/Predicate"
	export { isTagged as reexported } from "effect/Predicate"
	import { reexported } from "./test.js"
	const alias = tagged
	const aliasChain = alias
	let mutable = tagged
	const applied = tagged("Foo")
	const fake = { isTagged: (_value: unknown, _tag: string) => true }
	type Tagged = { readonly _tag: "Foo" } | { readonly _tag: "Bar" }
	declare const expectedTag: "Foo" | "Bar"
	declare const args: [Tagged, "Foo"]
	declare const ready: boolean
"#;

// go test: internal/typeparser/predicate_type_test.go:29 TestIsNodeReferenceToEffectPredicateModuleApi e570823bc6d9a4f0
#[test]
fn test_is_node_reference_to_effect_predicate_module_api() {
    run(|t| {
        t.parallel();
        for version in [EFFECT_V3, EFFECT_V4] {
            t.run(version, |t| {
                t.parallel();
                let source = format!(
                    "{PREDICATE_TEST_IMPORTS}{}",
                    r#"
				const root = (value: Tagged) => Predicate.isTagged(value, "Foo")
				const subpath = (value: Tagged) => P.isTagged(value, "Foo")
				const renamed = (value: Tagged) => tagged(value, "Foo")
				const exported = (value: Tagged) => reexported(value, "Foo")
				const constant = (value: Tagged) => aliasChain(value, "Foo")
				const mutableAlias = (value: Tagged) => mutable(value, "Foo")
				const lookalike = (value: Tagged) => fake.isTagged(value, "Foo")
				const shadowed = (Predicate: typeof fake) => Predicate.isTagged({}, "Foo")
				const otherExport = (value: Tagged) => P.hasProperty(value, "_tag")
			"#
                );
                let (mut compiled, sf) =
                    compile_and_get_checker_and_source_file_with_effect_version_internal(
                        t, version, &source,
                    );
                let mut tp = compiled.type_parser();
                struct Test {
                    name: &'static str,
                    member: &'static str,
                    want: bool,
                }
                for tt in [
                    Test { name: "root", member: "isTagged", want: true },
                    Test { name: "subpath", member: "isTagged", want: true },
                    Test { name: "renamed", member: "isTagged", want: true },
                    Test { name: "exported", member: "isTagged", want: true },
                    Test { name: "constant", member: "isTagged", want: true },
                    Test { name: "mutableAlias", member: "isTagged", want: false },
                    Test { name: "lookalike", member: "isTagged", want: false },
                    Test { name: "shadowed", member: "isTagged", want: false },
                    Test { name: "otherExport", member: "isTagged", want: false },
                    Test { name: "otherExport", member: "hasProperty", want: true },
                ] {
                    let function = find_returning_dispatch_test_function(t, sf, tt.name);
                    let callee =
                        get_function_like_body(function).unwrap().as_call_expression().expression();
                    let got =
                        tp.is_node_reference_to_effect_predicate_module_api(callee, tt.member);
                    if got != tt.want {
                        t.error(sprintf!(
                            "%s as %s = %v, want %v",
                            tt.name,
                            tt.member,
                            got,
                            tt.want
                        ));
                    }
                }
            });
        }
    });
}

// go test: internal/typeparser/predicate_type_test.go:72 TestParseTagMatchPredicate 348b3936153171b3
#[test]
fn test_parse_tag_match_predicate() {
    run(|t| {
        t.parallel();
        struct Test {
            name: &'static str,
            expression: &'static str,
            subject: &'static str,
            tag: &'static str,
        }
        let test = |name, expression, subject, tag| Test { name, expression, subject, tag };
        let tests = [
            test("direct", r#"Predicate.isTagged(value, "Foo")"#, "value", r#""Foo""#),
            test("curried", r#"Predicate.isTagged("Foo")(value)"#, "value", r#""Foo""#),
            test("namespace", r#"P.isTagged(value, "Foo")"#, "value", r#""Foo""#),
            test("renamed", r#"tagged("Foo")(value)"#, "value", r#""Foo""#),
            test("reexported", r#"reexported(value, "Foo")"#, "value", r#""Foo""#),
            test("constant alias", r#"aliasChain("Foo")(value)"#, "value", r#""Foo""#),
            test(
                "transparent direct",
                r#"((Predicate.isTagged! as typeof tagged)((value as Tagged)!, ("Foo" satisfies string)))"#,
                "value",
                r#""Foo""#,
            ),
            test(
                "transparent curried",
                r#"((<typeof tagged>Predicate.isTagged)(("Foo" as const)) as (self: unknown) => boolean)((value satisfies Tagged)!)"#,
                "value",
                r#""Foo""#,
            ),
            test(
                "nested subject",
                r#"Predicate.isTagged({ reason: value }.reason, "Foo")"#,
                "{ reason: value }.reason",
                r#""Foo""#,
            ),
            test(
                "dynamic direct tag",
                "Predicate.isTagged(value, expectedTag)",
                "value",
                "expectedTag",
            ),
            test(
                "dynamic curried tag",
                "Predicate.isTagged(expectedTag)(value)",
                "value",
                "expectedTag",
            ),
            test("zero arguments", "Predicate.isTagged()", "", ""),
            test("unapplied", r#"Predicate.isTagged("Foo")"#, "", ""),
            test("extra direct argument", r#"Predicate.isTagged(value, "Foo", ready)"#, "", ""),
            test("zero outer arguments", r#"Predicate.isTagged("Foo")()"#, "", ""),
            test("extra outer argument", r#"Predicate.isTagged("Foo")(value, other)"#, "", ""),
            test("extra inner argument", r#"Predicate.isTagged("Foo", "Bar")(value)"#, "", ""),
            test("zero inner arguments", "Predicate.isTagged()(value)", "", ""),
            test("spread subject", r#"Predicate.isTagged(...args, "Foo")"#, "", ""),
            test("spread tag", r#"Predicate.isTagged(value, ...["Foo"])"#, "", ""),
            test("spread inner", r#"Predicate.isTagged(...["Foo"])(value)"#, "", ""),
            test("spread outer", r#"Predicate.isTagged("Foo")(...[value])"#, "", ""),
            test("optional direct call", r#"Predicate.isTagged?.(value, "Foo")"#, "", ""),
            test("optional module access", r#"Predicate?.isTagged(value, "Foo")"#, "", ""),
            test("parenthesized optional access", r#"(Predicate?.isTagged)(value, "Foo")"#, "", ""),
            test("optional inner call", r#"Predicate.isTagged?.("Foo")(value)"#, "", ""),
            test("optional outer call", r#"Predicate.isTagged("Foo")?.(value)"#, "", ""),
            test("negation", r#"!Predicate.isTagged(value, "Foo")"#, "", ""),
            test("compound", r#"Predicate.isTagged(value, "Foo") && ready"#, "", ""),
            test("boolean equality", r#"Predicate.isTagged(value, "Foo") === true"#, "", ""),
            test("lookalike", r#"fake.isTagged(value, "Foo")"#, "", ""),
            test("mutable alias", r#"mutable(value, "Foo")"#, "", ""),
            test("applied predicate alias", "applied(value)", "", ""),
            test(
                "wrapper function",
                r#"((self: Tagged, tag: "Foo") => tagged(self, tag))(value, "Foo")"#,
                "",
                "",
            ),
            test("different export", r#"Predicate.hasProperty(value, "_tag")"#, "", ""),
        ];
        let mut source = String::new();
        source.push_str(PREDICATE_TEST_IMPORTS);
        for (i, tt) in tests.iter().enumerate() {
            source.push_str(&sprintf!(
                "\nconst match%d = (value: Tagged, other: Tagged) => %s\n",
                i as i64,
                tt.expression
            ));
        }
        for version in [EFFECT_V3, EFFECT_V4] {
            t.run(version, |t| {
                t.parallel();
                let (mut compiled, sf) =
                    compile_and_get_checker_and_source_file_with_effect_version_internal(
                        t, version, &source,
                    );
                let mut tp = compiled.type_parser();
                for (i, tt) in tests.iter().enumerate() {
                    t.run(tt.name, |t| {
                        let function = find_returning_dispatch_test_function(
                            t,
                            sf,
                            &sprintf!("match%d", i as i64),
                        );
                        let body = get_function_like_body(function);
                        let (subject, tag) = tp.parse_tag_match(body);
                        let got = returning_dispatch_node_text(Some(sf), subject);
                        if got != tt.subject {
                            t.error(sprintf!("subject = %q, want %q", got, tt.subject));
                        }
                        let got = returning_dispatch_node_text(Some(sf), tag);
                        if got != tt.tag {
                            t.error(sprintf!("tag = %q, want %q", got, tt.tag));
                        }
                        for node in [subject, tag] {
                            if node.is_some() && !predicate_node_descends_from(node, body) {
                                t.error("tag evidence must preserve an original condition node");
                            }
                        }
                    });
                }
            });
        }
    });
}

// go test: internal/typeparser/predicate_type_test.go:148 TestParseResultDispatchPredicateHints be81a7e18498531a
#[test]
fn test_parse_result_dispatch_predicate_hints() {
    run(|t| {
        t.parallel();
        for version in [EFFECT_V3, EFFECT_V4] {
            t.run(version, |t| {
                t.parallel();
                let source = format!(
                    "{PREDICATE_TEST_IMPORTS}{}",
                    r#"
				type Wrapper = { readonly reason: Tagged; readonly alternateReason: Tagged }
				declare const other: Wrapper
				const mixed = (value: Tagged) =>
					Predicate.isTagged(value, "Foo") ? 1 : value._tag === "Bar" ? 2 : 0
				const nested = (error: Wrapper) => {
					if (error.reason._tag === "Foo") return 1
					if (Predicate.isTagged("Bar")(error.reason)) return 2
					return 0
				}
				const wrapped = (value: Tagged) =>
					(Predicate.isTagged((value satisfies Tagged), ("Foo" as const))) ? 1 : Predicate.isTagged("Bar")(value) ? 2 : 0
				const differentRoot = (error: Wrapper) =>
					Predicate.isTagged(error.reason, "Foo") ? 1 : Predicate.isTagged(other.reason, "Bar") ? 2 : 0
				const differentProperty = (error: Wrapper) =>
					Predicate.isTagged(error.reason, "Foo") ? 1 : Predicate.isTagged(error.alternateReason, "Bar") ? 2 : 0
				const partlyUntagged = (value: Tagged) =>
					Predicate.isTagged(value, "Foo") ? 1 : fake.isTagged(value, "Bar") ? 2 : 0
			"#
                );
                let (mut compiled, sf) =
                    compile_and_get_checker_and_source_file_with_effect_version_internal(
                        t, version, &source,
                    );
                let mut tp = compiled.type_parser();
                struct Test {
                    name: &'static str,
                    subject: &'static str,
                }
                for tt in [
                    Test { name: "mixed", subject: "value" },
                    Test { name: "nested", subject: "error.reason" },
                    Test { name: "wrapped", subject: "value" },
                    Test { name: "differentRoot", subject: "" },
                    Test { name: "differentProperty", subject: "" },
                    Test { name: "partlyUntagged", subject: "" },
                ] {
                    let function = find_returning_dispatch_test_function(t, sf, tt.name);
                    let Some(returning) = tp.parse_returning_dispatch(function) else {
                        t.fatal(sprintf!("%s: expected returning dispatch", tt.name));
                    };
                    let dispatches =
                        [Some(returning.dispatch), tp.parse_result_dispatch(get_function_like_body(function))];
                    for dispatch in &dispatches {
                        let got = returning_dispatch_node_text(Some(sf),
                            ResultDispatch::common_tag_subject(dispatch.as_ref(), &mut tp),
                        );
                        if got != tt.subject {
                            t.error(sprintf!(
                                "%s: common subject = %q, want %q",
                                tt.name,
                                got,
                                tt.subject
                            ));
                        }
                        // Rust: Go reads the branches through the dispatch pointer, which panics when it is nil.
                        for branch in &dispatch.as_ref().unwrap().branches {
                            let condition = branch.condition;
                            if condition.source != condition.subject
                                || condition.value.is_some()
                                || !predicate_node_descends_from(
                                    Some(condition.source),
                                    get_function_like_body(function),
                                )
                            {
                                t.error(sprintf!(
                                    "%s: predicate source evidence changed",
                                    tt.name
                                ));
                            }
                            if condition.tag_subject.is_some()
                                && (!predicate_node_descends_from(
                                    condition.tag_subject,
                                    Some(condition.source),
                                ) || !predicate_node_descends_from(
                                    condition.tag_value,
                                    Some(condition.source),
                                ))
                            {
                                t.error(sprintf!(
                                    "%s: tag evidence is not from the original condition",
                                    tt.name
                                ));
                            }
                        }
                    }
                }
                for name in ["mixed", "wrapped"] {
                    let function = find_returning_dispatch_test_function(t, sf, name);
                    let original =
                        get_function_like_body(function).unwrap().as_conditional_expression().condition();
                    let condition =
                        tp.parse_returning_dispatch(function).unwrap().dispatch.branches[0].condition;
                    let call =
                        unwrap_result_dispatch_expression(Some(original)).unwrap().as_call_expression();
                    let arguments = call.arguments().nodes();
                    if condition.source != original
                        || condition.subject != original
                        || condition.tag_subject != unwrap_result_dispatch_expression(Some(arguments[0]))
                        || condition.tag_value != unwrap_result_dispatch_expression(Some(arguments[1]))
                    {
                        t.error(sprintf!(
                            "%s: Predicate call must preserve the exact condition and unwrapped argument nodes",
                            name
                        ));
                    }
                }
            });
        }
    });
}

// go test: internal/typeparser/predicate_type_test.go:221 predicateNodeDescendsFrom 6c136ceadf054c7d
fn predicate_node_descends_from(node: Option<P<Node>>, ancestor: Option<P<Node>>) -> bool {
    let mut node = node;
    while let Some(current) = node {
        if Some(current) == ancestor {
            return true;
        }
        node = current.parent();
    }
    false
}
