use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::checker::Type;
use ets_shim::core::P;
use rustc_hash::FxHashMap;

use crate::TypeParser;
use crate::helpers_test::*;
use crate::module_export_reference_test::kind_string;

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:12 firstNodeOfKind 443f30d1738d924f
fn first_node_of_kind(t: &T, sf: P<SourceFile>, kind: Kind) -> P<Node> {
    t.helper();
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

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:30 TestNodeCannotBeEffectLibraryTypeObjectLiteralsOnlyInV4 35f09a0dec9f4ada
#[test]
fn test_node_cannot_be_effect_library_type_object_literals_only_in_v4() {
    run(|t| {
        t.parallel();
        let source = "import { Effect } from \"effect\"
export const value = { a: 1 }
export const effect = Effect.void";
        for (version, expected) in [(EFFECT_V4, true), (EFFECT_V3, false)] {
            if let Err(err) = ensure_package_installed(version, "effect") {
                t.skip(sprintf!("Effect not installed: %v", err));
            }
            let (mut compiled, sf) =
                compile_and_get_checker_and_source_file_with_effect_version_internal(
                    t, version, source,
                );
            let mut tp = compiled.type_parser();
            let got = tp.node_cannot_be_effect_library_type(first_node_of_kind(
                t,
                sf,
                Kind::ObjectLiteralExpression,
            ));
            if got != expected {
                t.error(sprintf!(
                    "%v: nodeCannotBeEffectLibraryType(object literal) = %v, want %v",
                    version,
                    got,
                    expected
                ));
            }
        }
    });
}

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:53 TestNodeCouldBeEffectKeepsNarrowedReferences 9fcaf6f4c42005b1
#[test]
fn test_node_could_be_effect_keeps_narrowed_references() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V4, "effect") {
            t.skip(sprintf!("Effect v4 not installed: %v", err));
        }
        let source = r#"
import { Effect } from "effect"
interface Plain { readonly value: number }
declare const guarded: Plain
export const narrowed = Effect.isEffect(guarded) ? guarded : null
"#;
        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let narrowed = first_node_of_kind(t, sf, Kind::ConditionalExpression)
            .as_conditional_expression()
            .when_true();
        let narrowed_type = tp.get_type_at_location(narrowed);
        if tp.effect_type(narrowed_type).is_none() {
            t.fatal("fixture: the guarded reference should narrow to an Effect");
        }
        if !tp.node_could_be_effect(narrowed) {
            t.error("a reference narrowed by a type guard must not be ruled out");
        }
    });
}

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:76 TestNodeCannotBeEffectLibraryTypePrimitiveExpressions d042a0e5c46c9a25
#[test]
fn test_node_cannot_be_effect_library_type_primitive_expressions() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V4, "effect") {
            t.skip(sprintf!("Effect v4 not installed: %v", err));
        }
        let source = "import { Effect } from \"effect\"
declare const x: number
export const trueLiteral = true
export const falseLiteral = false
export const nullLiteral = null
export const regex = /a/
export const not = !x
export const negate = -x
export const voided = void 0
export const typeOf = typeof x
export const undefinedReference = undefined
export const effect = Effect.void";
        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let mut initializers: FxHashMap<String, Option<P<Node>>> = FxHashMap::default();
        for &statement in sf.statements().nodes() {
            if statement.kind() != Kind::VariableStatement {
                continue;
            }
            let declaration_list = statement.as_variable_statement().declaration_list();
            for &declaration in
                declaration_list.as_variable_declaration_list().declarations().nodes()
            {
                let name = declaration.name().expect("a variable declaration has a name");
                initializers.insert(name.text().to_string(), declaration.initializer());
            }
        }
        // Rust: Go ranges over a map literal (random order); the cases only report errors, so a fixed order reports
        // the same set.
        for (name, expected) in [
            ("trueLiteral", true),
            ("falseLiteral", true),
            ("nullLiteral", true),
            ("regex", true),
            ("not", true),
            ("negate", true),
            ("voided", true),
            ("typeOf", true),
            ("undefinedReference", false),
            ("effect", false),
        ] {
            // Go: a missing name reads a nil node, which nodeCannotBeEffectLibraryType dereferences.
            let node = initializers
                .get(name)
                .copied()
                .flatten()
                .expect("every declaration of the fixture has an initializer");
            let got = tp.node_cannot_be_effect_library_type(node);
            if got != expected {
                t.error(sprintf!(
                    "nodeCannotBeEffectLibraryType(%s) = %v, want %v",
                    name,
                    got,
                    expected
                ));
            }
            let node_type = tp.get_type_at_location(node);
            let is_effect = tp.effect_type(node_type).is_some();
            if expected && is_effect {
                t.error(sprintf!("fixture: %s was ruled out but its type is an Effect", name));
            }
        }
    });
}

// Rust: Go's parsers return `any`; the Rust results are typed, so they are wrapped in one comparable enum.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ParsedType {
    Effect(crate::Effect),
    Layer(crate::Layer),
    Schema(crate::SchemaTypes),
    Service(crate::Service),
    /// A boolean parser's `true`.
    Flag,
}

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:129 atLocationParser 19214ce229bc74e7
// atLocationParser pairs an AtLocation parser with the parser it applies to
// the type of a node; each reports its result and whether it is non-nil.
// Rust: the closures take the parser as an argument instead of capturing it, and a result is non-nil when it is
// `Some`.
struct AtLocationParser {
    name: &'static str,
    at_location: fn(&mut TypeParser<'_>, Option<P<Node>>) -> Option<ParsedType>,
    parse: fn(&mut TypeParser<'_>, Option<P<Type>>) -> Option<ParsedType>,
}

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:135 atLocationParsers e7030614e51afe5d
fn at_location_parsers() -> Vec<AtLocationParser> {
    vec![
        AtLocationParser {
            name: "Effect",
            at_location: |tp, n| tp.effect_type_at_location(n).map(ParsedType::Effect),
            parse: |tp, t| tp.effect_type(t).map(ParsedType::Effect),
        },
        AtLocationParser {
            name: "StrictEffect",
            at_location: |tp, n| tp.strict_effect_type_at_location(n).map(ParsedType::Effect),
            parse: |tp, t| tp.strict_effect_type(t).map(ParsedType::Effect),
        },
        AtLocationParser {
            name: "EffectTypeId",
            at_location: |tp, n| tp.has_effect_type_id_at_location(n).then_some(ParsedType::Flag),
            parse: |tp, t| tp.has_effect_type_id(t).then_some(ParsedType::Flag),
        },
        AtLocationParser {
            name: "Stream",
            at_location: |tp, n| tp.stream_type_at_location(n).map(ParsedType::Effect),
            parse: |tp, t| tp.stream_type(t).map(ParsedType::Effect),
        },
        AtLocationParser {
            name: "Layer",
            at_location: |tp, n| tp.layer_type_at_location(n).map(ParsedType::Layer),
            parse: |tp, t| tp.layer_type(t).map(ParsedType::Layer),
        },
        AtLocationParser {
            name: "Schema",
            at_location: |tp, n| tp.effect_schema_types_at_location(n).map(ParsedType::Schema),
            parse: |tp, t| tp.effect_schema_types(t).map(ParsedType::Schema),
        },
        AtLocationParser {
            name: "Service",
            at_location: |tp, n| tp.service_type_at_location(n).map(ParsedType::Service),
            parse: |tp, t| tp.service_type(t).map(ParsedType::Service),
        },
        AtLocationParser {
            name: "ContextTag",
            at_location: |tp, n| tp.context_tag_at_location(n).map(ParsedType::Service),
            parse: |tp, t| tp.context_tag(t).map(ParsedType::Service),
        },
        AtLocationParser {
            name: "Yieldable",
            at_location: |tp, n| tp.effect_yieldable_type_at_location(n).map(ParsedType::Effect),
            parse: |tp, t| tp.effect_yieldable_type(t).map(ParsedType::Effect),
        },
    ]
}

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:158 exportedInitializers 07beef450b626781
fn exported_initializers(sf: P<SourceFile>) -> FxHashMap<String, Option<P<Node>>> {
    let mut initializers = FxHashMap::default();
    for &statement in sf.statements().nodes() {
        if statement.kind() != Kind::VariableStatement {
            continue;
        }
        let declaration_list = statement.as_variable_statement().declaration_list();
        for &declaration in declaration_list.as_variable_declaration_list().declarations().nodes() {
            let name = declaration.name().expect("a variable declaration has a name");
            initializers.insert(name.text().to_string(), declaration.initializer());
        }
    }
    initializers
}

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:175 checkAtLocationParsers 8e5ee70b5ada5c3c
// checkAtLocationParsers asserts that every AtLocation parser matches its
// parser applied to GetTypeAtLocation, that the parsers named in expected
// (and only those) succeed, and that a node ruled out by every parser that
// skips it is never typed.
// Rust: Go ranges over the expected map (random order); a slice reports the same set of errors.
fn check_at_location_parsers(
    t: &T,
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    expected: &[(&str, &[&str])],
) {
    t.helper();
    let initializers = exported_initializers(sf);
    let parsers = at_location_parsers();
    for &(name, accepted) in expected {
        let Some(node) = initializers.get(name).copied().flatten() else {
            t.fatal(sprintf!("no initializer for %s", name));
        };
        let mut results: FxHashMap<&str, Option<ParsedType>> = FxHashMap::default();
        for parser in &parsers {
            let result = (parser.at_location)(tp, Some(node));
            results.insert(parser.name, result);
            let ok = result.is_some();
            let want = accepted.contains(&parser.name);
            if ok != want {
                t.error(sprintf!(
                    "%s: %sAtLocation succeeded = %v, want %v",
                    name,
                    parser.name,
                    ok,
                    want
                ));
            }
        }
        if tp.node_cannot_be_effect_library_type(node)
            && node.kind() != Kind::ObjectLiteralExpression
            && tp.links().type_at_location.contains_key(&node)
        {
            t.error(sprintf!("%s: a ruled-out node was typed", name));
        }
        for parser in &parsers {
            let node_type = tp.get_type_at_location(node);
            let want = (parser.parse)(tp, node_type);
            if results[parser.name] != want {
                t.error(sprintf!(
                    "%s: %sAtLocation differs from the parser on GetTypeAtLocation",
                    name,
                    parser.name
                ));
            }
        }
    }
}

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:203 TestAtLocationParsersV4 deae00ec468c8cdd
#[test]
fn test_at_location_parsers_v4() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V4, "effect") {
            t.skip(sprintf!("Effect v4 not installed: %v", err));
        }
        let (mut compiled, sf) = compile_and_get_checker_and_source_file_with_effect_v4_internal(
            t,
            r#"import { Context, Effect, Layer, Schema, Stream } from "effect"
class UserRepo extends Context.Service<UserRepo, { readonly find: () => string }>()("UserRepo") {}
export const effect = Effect.succeed(1)
export const layer = Layer.empty
export const stream = Stream.make(1)
export const schema = Schema.String
export const service = UserRepo
export const yieldable = { asEffect: () => Effect.succeed(1) }
export const object = { value: 1 }
export const arrow = () => Effect.succeed(1)
export const number = 1"#,
        );
        let mut tp = compiled.type_parser();

        check_at_location_parsers(
            t,
            &mut tp,
            sf,
            &[
                ("effect", &["Effect", "EffectTypeId", "StrictEffect", "Yieldable"]),
                ("layer", &["Layer"]),
                ("stream", &["Stream"]),
                ("schema", &["Schema"]),
                ("service", &["Service", "Effect", "EffectTypeId", "Yieldable"]),
                ("yieldable", &["Yieldable"]),
                ("object", &[]),
                ("arrow", &[]),
                ("number", &[]),
            ],
        );
    });
}

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:234 TestAtLocationParsersV3 b0cacb5d2e2fbfeb
#[test]
fn test_at_location_parsers_v3() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V3, "effect") {
            t.skip(sprintf!("Effect v3 not installed: %v", err));
        }
        let (mut compiled, sf) = compile_and_get_checker_and_source_file_with_effect_v3_internal(
            t,
            r#"import { Context, Effect, Layer, Schema, Stream } from "effect"
class Config extends Context.Tag("Config")<Config, { readonly port: number }>() {}
export const effect = Effect.succeed(1)
export const layer = Layer.empty
export const stream = Stream.make(1)
export const schema = Schema.String
export const tag = Config
export const object = { value: 1 }
export const arrow = () => Effect.succeed(1)
export const number = 1"#,
        );
        let mut tp = compiled.type_parser();

        check_at_location_parsers(
            t,
            &mut tp,
            sf,
            &[
                // v3 Streams carry the Effect variance struct, so the two overlap.
                ("effect", &["Effect", "EffectTypeId", "StrictEffect", "Stream", "Yieldable"]),
                ("layer", &["Layer"]),
                ("stream", &["Stream", "Effect", "EffectTypeId", "Yieldable"]),
                ("schema", &["Schema"]),
                ("tag", &["ContextTag", "Effect", "EffectTypeId", "Stream", "Yieldable"]),
                ("object", &[]),
                ("arrow", &[]),
                ("number", &[]),
            ],
        );
    });
}

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:264 TestAtLocationParsersAreNilSafe 832fa13e22b81454
// Rust: a TypeParser is never nil nor without a checker, so only the nil node case is ported, on a parser of an
// empty program.
#[test]
fn test_at_location_parsers_are_nil_safe() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V4, "effect") {
            t.skip(sprintf!("Effect v4 not installed: %v", err));
        }
        let (mut compiled, _) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, "export {}");
        let mut tp = compiled.type_parser();
        for parser in at_location_parsers() {
            if (parser.at_location)(&mut tp, None).is_some() {
                t.error(sprintf!("%sAtLocation(nil) succeeded", parser.name));
            }
        }
    });
}
