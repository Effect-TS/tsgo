package rules_test

import (
	"regexp"
	"slices"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

func TestFailCauseToConstructorShapes(t *testing.T) {
	t.Parallel()
	source := `import { Cause, Effect, pipe } from "effect"
declare const error: Error
declare const defect: unknown
declare const wrap: (e: Error) => Error
export const failCall = Effect.failCause(Cause.fail(error))
export const dieCall = Effect.failCause(Cause.die(defect))
export const longerPipe = pipe(error, wrap, Cause.fail, Effect.failCause)
export const effectFnTrailing = Effect.fn(function*() {
  return yield* Effect.succeed(error)
}, Cause.fail, Effect.failCause)
export const lazyFail = Effect.failCauseSync(() => Cause.fail(error))
export const lazyDie = Effect.failCauseSync(() => Cause.die(defect))
export const lazyTypeArgument = Effect.failCauseSync(() => Cause.fail<Error | string>(error))
export const asyncThunk = Effect.failCauseSync(async () => Cause.fail(error))
export const generatorThunk = Effect.failCauseSync(function*() {
  return Cause.fail(error)
} as any)
export const parameterThunk = Effect.failCauseSync(((_?: unknown) => Cause.fail(error)) as () => Cause.Cause<Error>)
`
	exportName := regexp.MustCompile(`export const (\w+)`)
	messages := map[string]string{
		"failCall":         "`Effect.fail` expresses this failure directly instead of wrapping `Cause.fail` in `Effect.failCause`.",
		"dieCall":          "`Effect.die` expresses this failure directly instead of wrapping `Cause.die` in `Effect.failCause`.",
		"longerPipe":       "`Effect.fail` expresses this failure directly instead of wrapping `Cause.fail` in `Effect.failCause`.",
		"effectFnTrailing": "`Effect.fail` expresses this failure directly instead of wrapping `Cause.fail` in `Effect.failCause`.",
		"lazyFail":         "`Effect.failSync` expresses this failure directly instead of wrapping `Cause.fail` in `Effect.failCauseSync`.",
		"lazyDie":          "`Effect.dieSync` expresses this failure directly instead of wrapping `Cause.die` in `Effect.failCauseSync`.",
		"lazyTypeArgument": "`Effect.failSync` expresses this failure directly instead of wrapping `Cause.fail` in `Effect.failCauseSync`.",
	}

	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			want := []string{"failCall", "dieCall", "longerPipe", "effectFnTrailing", "lazyFail", "lazyDie", "lazyTypeArgument"}
			if version == bundledeffect.EffectV4 {
				// Effect v4 has no Effect.dieSync to suggest.
				want = slices.DeleteFunc(want, func(name string) bool { return name == "lazyDie" })
			}
			preview := evaluatePreview(t, version, source, &rules.FailCauseToConstructor)
			var got []string
			for _, diagnostic := range preview.Diagnostics {
				lineStart := strings.LastIndex(preview.SourceText[:diagnostic.Start], "\n") + 1
				name := ""
				if match := exportName.FindStringSubmatch(preview.SourceText[lineStart:diagnostic.Start]); match != nil {
					name = match[1]
				} else if strings.HasPrefix(preview.SourceText[lineStart:], "}, Cause.fail, Effect.failCause)") {
					name = "effectFnTrailing"
				} else {
					t.Fatalf("diagnostic at %d is not on an export line", diagnostic.Start)
				}
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
