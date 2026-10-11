package rules_test

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

func TestFlatMapToTapMalformedCallbacks(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			preview := evaluatePreview(t, version, `import { Effect } from "effect"
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
`, &rules.FlatMapToTap)
			if len(preview.Diagnostics) != 1 {
				t.Fatalf("expected only the valid callback to match, got %d diagnostics", len(preview.Diagnostics))
			}
		})
	}
}
