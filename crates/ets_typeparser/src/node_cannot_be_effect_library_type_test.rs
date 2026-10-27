use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::core::P;
use rustc_hash::FxHashMap;

use crate::helpers_test::*;
use crate::module_export_reference_test::kind_string;

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:10 firstNodeOfKind 443f30d1738d924f
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

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:28 TestNodeCannotBeEffectLibraryTypeObjectLiteralsOnlyInV4 5898e1073d9100a4
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
                    "%v: NodeCannotBeEffectLibraryType(object literal) = %v, want %v",
                    version,
                    got,
                    expected
                ));
            }
        }
    });
}

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:51 TestNodeCouldBeEffectKeepsNarrowedReferences 9fcaf6f4c42005b1
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

// go test: internal/typeparser/node_cannot_be_effect_library_type_test.go:74 TestNodeCannotBeEffectLibraryTypePrimitiveExpressions 749fdb3d01524547
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
            // Go: a missing name reads a nil node, which NodeCannotBeEffectLibraryType dereferences.
            let node = initializers
                .get(name)
                .copied()
                .flatten()
                .expect("every declaration of the fixture has an initializer");
            let got = tp.node_cannot_be_effect_library_type(node);
            if got != expected {
                t.error(sprintf!(
                    "NodeCannotBeEffectLibraryType(%s) = %v, want %v",
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
