package rules_test

import (
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

func TestFlatMapIdentityToFlattenCallbackShapes(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			preview := evaluatePreview(t, version, `import { Channel, Effect, identity } from "effect"
import { flatMap } from "effect/Effect"
declare const source: Effect.Effect<Effect.Effect<string>>
declare const fallback: Effect.Effect<string>
export const valid = source.pipe(Effect.flatMap((inner) => inner))
export const namedImport = source.pipe(flatMap((inner) => inner))
export const returnType = source.pipe(Effect.flatMap((inner): Effect.Effect<string> => inner))
export const widened = source.pipe(Effect.flatMap((inner: Effect.Effect<string, unknown>) => inner))
export const typeArguments = source.pipe(Effect.flatMap<Effect.Effect<string>, string, never, never>(identity))
export const asyncCallback = source.pipe(Effect.flatMap(async (inner) => inner))
export const twoParameters = source.pipe(Effect.flatMap((inner, _extra?: number) => inner))
export const restParameter = source.pipe(Effect.flatMap((...inner: [Effect.Effect<string>]) => inner))
export const initializer = source.pipe(Effect.flatMap((inner = fallback) => inner))
export const optionalParameter = source.pipe(Effect.flatMap((inner?: Effect.Effect<string>) => inner))
export const destructured = source.pipe(Effect.flatMap(({ pipe }) => pipe()))
export const channelIdentity = source.pipe(Effect.flatMap(Channel.identity))
`, &rules.FlatMapIdentityToFlatten)
			if len(preview.Diagnostics) != 2 {
				t.Fatalf("expected only the valid and named-import callbacks to match, got %d diagnostics: %+v", len(preview.Diagnostics), preview.Diagnostics)
			}
			for i, want := range []string{"Effect.flatMap", "flatMap"} {
				if got := preview.SourceText[preview.Diagnostics[i].Start:preview.Diagnostics[i].End]; got != want {
					t.Errorf("diagnostic %d covers %q, want %q", i, got, want)
				}
			}
			if line := strings.Count(preview.SourceText[:preview.Diagnostics[1].Start], "\n"); line != 5 {
				t.Errorf("named-import diagnostic on line %d, want 5", line)
			}
		})
	}
}
