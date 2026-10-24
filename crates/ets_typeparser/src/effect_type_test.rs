// Ported from effect-tsgo's internal/typeparser/effect_type_test.go (see helpers_test.rs).

use ets_go::sprintf;
use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::helpers_test::*;

// go test: internal/typeparser/effect_type_test.go:9 TestStrictEffectTypeCachesOnlyEffectNamedTypes
// Rust: Go compares the cached `*Effect` by pointer; the Rust result is a value, compared by value.
#[test]
fn test_strict_effect_type_caches_only_effect_named_types() {
    run(|t| {
        t.parallel();

        let (mut compiled, sf) = compile_and_get_checker_and_source_file_with_effect_v4_internal(
            t,
            r#"import { Effect, Layer, Stream } from "effect"
export const effect = Effect.succeed(1)
export const stream = Stream.make(1)
export const layer = Layer.empty
export const number = 1
export const object = { value: 1 }
"#,
        );
        let mut tp = compiled.type_parser();

        let type_of = |tp: &mut crate::TypeParser<'_>, name: &str| -> P<Type> {
            let Some(typ) = tp.get_type_at_location(find_identifier_by_text(t, sf, name, 0)) else {
                t.fatal(sprintf!("expected a type for %s", name));
            };
            typ
        };

        for name in ["stream", "layer", "number", "object"] {
            let typ = type_of(&mut tp, name);
            for _ in 0..2 {
                if tp.strict_effect_type(typ).is_some() {
                    t.error(sprintf!("%s: expected no strict Effect type", name));
                }
            }
            if tp.links().strict_effect_type.contains_key(&typ) {
                t.error(sprintf!("%s: rejected type has a cache entry", name));
            }
        }
        let typ = type_of(&mut tp, "effect");
        let Some(effect) = tp.strict_effect_type(typ) else {
            t.fatal("effect: expected a strict Effect type");
        };
        let got = type_string(&mut tp, effect.a());
        if got != "number" {
            t.error(sprintf!("effect: success type %q, want number", got));
        }
        if tp.links().strict_effect_type.get(&typ) != Some(&Some(effect)) {
            t.fatal("effect: expected a cache entry holding the result");
        }
        if tp.strict_effect_type(typ) != Some(effect) {
            t.error("effect: expected the cached result on a repeated query");
        }
    });
}
