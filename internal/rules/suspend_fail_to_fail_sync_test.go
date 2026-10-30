package rules_test

import (
	"regexp"
	"slices"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

func TestSuspendFailToFailSyncShapes(t *testing.T) {
	t.Parallel()
	source := `import * as Effect from "effect/Effect"
declare const error: Error
declare const defect: unknown
declare const args: [Error]
export const failCall = Effect.suspend(() => Effect.fail(error))
export const dieCall = Effect.suspend(() => Effect.die(defect))
export const functionThunk = Effect.suspend(function() {
  return Effect.fail(error)
})
export const asyncThunk = Effect.suspend((async () => Effect.fail(error)) as any)
export const generatorThunk = Effect.suspend(function*() {
  return Effect.fail(error)
} as any)
export const typeParameterThunk = Effect.suspend(<T>() => Effect.fail(error))
export const spreadArgument = Effect.suspend(() => Effect.fail(...args))
export const emptyReturn = Effect.suspend((() => {
  return
}) as any)
`
	exportName := regexp.MustCompile(`export const (\w+)`)
	messages := map[string]string{
		"failCall":      "`Effect.failSync` builds this error lazily directly instead of wrapping `Effect.fail` in `Effect.suspend`.",
		"dieCall":       "`Effect.dieSync` builds this defect lazily directly instead of wrapping `Effect.die` in `Effect.suspend`.",
		"functionThunk": "`Effect.failSync` builds this error lazily directly instead of wrapping `Effect.fail` in `Effect.suspend`.",
	}

	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			want := []string{"failCall", "dieCall", "functionThunk"}
			if version == bundledeffect.EffectV4 {
				// Effect v4 has no Effect.dieSync to suggest.
				want = slices.DeleteFunc(want, func(name string) bool { return name == "dieCall" })
			}
			preview := evaluatePreview(t, version, source, &rules.SuspendFailToFailSync)
			var got []string
			for _, diagnostic := range preview.Diagnostics {
				lineStart := strings.LastIndex(preview.SourceText[:diagnostic.Start], "\n") + 1
				match := exportName.FindStringSubmatch(preview.SourceText[lineStart:diagnostic.Start])
				if match == nil {
					t.Fatalf("diagnostic at %d is not on an export line", diagnostic.Start)
				}
				name := match[1]
				if !strings.Contains(diagnostic.Text, messages[name]) {
					t.Fatalf("%s: unexpected diagnostic text %q", name, diagnostic.Text)
				}
				got = append(got, name)
			}
			if !slices.Equal(got, want) {
				t.Fatalf("matched %v, want %v", got, want)
			}
		})
	}
}
