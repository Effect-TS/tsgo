//! Port of `internal/rules/flat_map_to_tap_test.go`.

use ets_bundledeffect::EffectVersion;

use crate::*;

// go test: internal/rules/flat_map_to_tap_test.go:10 TestFlatMapToTapMalformedCallbacks
#[test]
fn test_flat_map_to_tap_malformed_callbacks() {
    run_on_checker_stack(|t| {
        t.parallel();
        for version in [EffectVersion::V3, EffectVersion::V4] {
            t.run(version.as_str(), |t| {
                t.parallel();
                let preview = evaluate_preview(
                    t,
                    version,
                    r#"import { Effect } from "effect"
declare const source: Effect.Effect<number>
declare const inner: unknown
declare const loose: any
export const valid = source.pipe(Effect.flatMap((n) => Effect.as(Effect.log(n), n)))
export const asyncCallback = source.pipe(Effect.flatMap(async (n) => Effect.as(Effect.log(n), n)))
export const generatorCallback = source.pipe(Effect.flatMap(function*(n) { return Effect.as(Effect.log(n), n) }))
export const twoParameters = source.pipe(Effect.flatMap((n, extra) => Effect.as(Effect.log(n), n)))
export const genericCallback = source.pipe(Effect.flatMap(<A>(n: A) => Effect.as(Effect.log(n), n)))
export const unknownInner = source.pipe(Effect.flatMap((n) => Effect.as(inner, n)))
export const looseInner = source.pipe(Effect.flatMap((n) => loose.pipe(Effect.as(n))))
export const looseOuter = loose.pipe(Effect.flatMap((n: number) => Effect.as(Effect.log(n), n)))
"#,
                    &ets_rules::FLAT_MAP_TO_TAP,
                );
                if preview.diagnostics.len() != 1 {
                    t.fatal(format!(
                        "expected only the valid callback to match, got {} diagnostics",
                        preview.diagnostics.len()
                    ));
                }
            });
        }
    });
}
