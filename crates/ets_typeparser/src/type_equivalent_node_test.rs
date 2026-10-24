use ets_go::{sprintf, sprintln};
use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;
use rustc_hash::FxHashMap;

use crate::helpers_test::*;
use crate::module_export_reference_test::{kind_string, node_pos};
use crate::*;

// go test: internal/typeparser/type_equivalent_node_test.go:10 TestTypeEquivalentNodeMatchesCheckerTypes
#[test]
fn test_type_equivalent_node_matches_checker_types() {
    run(|t| {
        t.parallel();
        if let Err(err) = ensure_package_installed(EFFECT_V4, "effect") {
            t.skip(sprintln!("Effect v4 not installed:", err));
        }
        let source = r#"
import { Effect } from "effect"
declare const service: { readonly run: Effect.Effect<number, string> }
export const viaAccess = service.run
export const wrapped = (Effect.succeed(1))
export const checked = Effect.fail("x") satisfies Effect.Effect<never, string>
export function f(param: Effect.Effect<number>) { return param }
export const literal = { inner: Effect.void, viaAccess }
export class C { field = Effect.never }
"#;
        let (mut compiled, sf) =
            compile_and_get_checker_and_source_file_with_effect_v4_internal(t, source);
        let mut tp = compiled.type_parser();

        let mut found: FxHashMap<Kind, i64> = FxHashMap::default();
        fn visit(
            t: &ets_go::testing::T,
            tp: &mut TypeParser<'_>,
            found: &mut FxHashMap<Kind, i64>,
            node: P<Node>,
        ) -> bool {
            let (target, ok) = type_equivalent_node(node);
            if ok {
                *found.entry(target.kind()).or_default() += 1;
                let got = tp.get_type_at_location(node);
                let want = tp.get_type_at_location(target);
                if got != want {
                    // Rust: Go's TypeToString of a nil type is what `type_string` of `None` stands for.
                    let got_text = got.map(|got| type_string(tp, got)).unwrap_or_default();
                    let want_text = want.map(|want| type_string(tp, want)).unwrap_or_default();
                    t.error(sprintf!(
                        "%v at %d: type %v differs from its equivalent %v: %v",
                        kind_string(node.kind()),
                        node_pos(node),
                        got_text,
                        kind_string(target.kind()),
                        want_text
                    ));
                }
            }
            node.for_each_child(&mut |child| visit(t, tp, found, child));
            false
        }
        sf.as_node().for_each_child(&mut |child| visit(t, &mut tp, &mut found, child));

        // Rust: Go ranges over a map literal (random order); the kinds are checked in a fixed order.
        for (kind, want) in [
            (Kind::PropertyAccessExpression, 5),
            (Kind::VariableDeclaration, 5),
            (Kind::Parameter, 1),
            (Kind::PropertyAssignment, 1),
            (Kind::ShorthandPropertyAssignment, 1),
            (Kind::PropertyDeclaration, 1),
            (Kind::CallExpression, 2),
        ] {
            let got = found.get(&kind).copied().unwrap_or(0);
            if got != want {
                t.error(sprintf!(
                    "equivalents of kind %v = %d, want %d",
                    kind_string(kind),
                    got,
                    want
                ));
            }
        }
    });
}
