package rules_test

import (
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

func TestMapMapErrorToMapBothMessagesAndLocations(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			source := `import { Effect } from "effect"
declare const source: Effect.Effect<number, string>
export const forward = source.pipe(Effect.map((n) => n + 1), Effect.mapError((e) => e.length))
export const reverse = source.pipe(Effect.mapError((e) => e.length), Effect.map((n) => n + 1))
export const nested = Effect.mapError(Effect.map(source, (n) => n + 1), (e) => e.length)
`
			expected := []struct{ location, first, second, replacement string }{
				{"Effect.mapError", "map", "mapError", "mapBoth"},
				{"Effect.map", "mapError", "map", "mapBoth"},
				{"Effect.mapError", "map", "mapError", "mapBoth"},
			}
			if version == bundledeffect.EffectV4 {
				source += "export const eager = source.pipe(Effect.mapErrorEager((e) => e.length), Effect.mapEager((n) => n + 1))\n"
				expected = append(expected, struct{ location, first, second, replacement string }{"Effect.mapEager", "mapErrorEager", "mapEager", "mapBothEager"})
			}
			preview := evaluatePreview(t, version, source, &rules.MapMapErrorToMapBoth)
			if len(preview.Diagnostics) != len(expected) {
				t.Fatalf("expected %d diagnostics, got %d", len(expected), len(preview.Diagnostics))
			}
			lines := strings.Split(source, "\n")
			for i, diagnostic := range preview.Diagnostics {
				want := expected[i]
				if got := preview.SourceText[diagnostic.Start:diagnostic.End]; got != want.location {
					t.Errorf("diagnostic %d is on %q, want %q", i, got, want.location)
				}
				if !strings.Contains(lines[i+2], preview.SourceText[diagnostic.Start:diagnostic.End]) {
					t.Errorf("diagnostic %d is not on line %q", i, lines[i+2])
				}
				message := "`Effect." + want.first + "` + `Effect." + want.second + "` transform the success and failure channels one step at a time; `Effect." + want.replacement + "` expresses both transformations in a single step."
				if !strings.Contains(diagnostic.Text, message) {
					t.Errorf("diagnostic %d message = %q, want %q", i, diagnostic.Text, message)
				}
			}
		})
	}
}

func TestMapMapErrorToMapBothBoundaries(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			preview := evaluatePreview(t, version, `import { Effect, pipe } from "effect"
declare const source: Effect.Effect<number, string>
declare const callbacks: [(n: number) => number]
export const control = source.pipe(Effect.map((n) => n + 1), Effect.mapError((e) => e.length))
export const nestedPipeCalls = pipe(pipe(source, Effect.map((n) => n + 1)), Effect.mapError((e) => e.length))
export const emptyPipeSubject = Effect.map(Effect.mapError(source, (e) => e.length).pipe(), (n) => n + 1)
export const spreadArgument = source.pipe(Effect.map(...callbacks), Effect.mapError((e) => e.length))
export const curried = Effect.map((n: number) => n + 1)(Effect.mapError((e: string) => e.length)(source))
export const untracedPipeline = Effect.fnUntraced(function*() {
  return yield* source
}, Effect.map((n) => n + 1), Effect.mapError((e) => e.length))
`, &rules.MapMapErrorToMapBoth)
			// Only the control pair may match.
			if len(preview.Diagnostics) != 1 || !strings.HasPrefix(preview.SourceText[preview.Diagnostics[0].Start:], "Effect.mapError((e) => e.length))\nexport const nestedPipeCalls") {
				for _, diagnostic := range preview.Diagnostics {
					t.Logf("diagnostic at offset %d on %q", diagnostic.Start, preview.SourceText[diagnostic.Start:diagnostic.End])
				}
				t.Fatalf("expected only the control diagnostic, got %d", len(preview.Diagnostics))
			}
		})
	}
}
