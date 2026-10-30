package rules_test

import (
	"regexp"
	"slices"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

func TestProvideMergeAllToArrayShapes(t *testing.T) {
	t.Parallel()
	source := `import { Context, Effect, Layer } from "effect"
class A extends Context.Tag("A")<A, { readonly a: number }>() {}
class B extends Context.Tag("B")<B, { readonly b: number }>() {}
class C extends Context.Tag("C")<C, { readonly c: number }>() {}
declare const ALayer: Layer.Layer<A>
declare const BLayer: Layer.Layer<B>
declare const CRequiresALayer: Layer.Layer<C, never, A>
declare const program: Effect.Effect<void, never, A | B>
export const effectFn = Effect.fn(function*() { yield* program }, Effect.provide(Layer.mergeAll(ALayer, BLayer)))
export const effectFnUntraced = Effect.fnUntraced(function*() { yield* program }, Effect.provide(Layer.mergeAll(ALayer, BLayer)))
export const unapplied = Effect.provide(Layer.mergeAll(ALayer, BLayer))
export const explicitMergeAllTypeArguments = program.pipe(Effect.provide(Layer.mergeAll<[Layer.Layer<A>, Layer.Layer<B>]>(ALayer, BLayer)))
export const explicitProvideTypeArguments = program.pipe(Effect.provide<A, never, never>(Layer.mergeAll(ALayer)))
// @ts-expect-error mergeAll requires at least one layer
export const noArguments = program.pipe(Effect.provide(Layer.mergeAll()))
export const dependentLayers = Effect.void.pipe(Effect.provide(Layer.mergeAll(ALayer, CRequiresALayer)))
`
	want := []string{"effectFn", "effectFnUntraced", "unapplied"}
	exportName := regexp.MustCompile(`export const (\w+)`)

	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			// Only provideMergeAllToArray runs, so the dependentLayers skip does
			// not rely on layerMergeAllWithDependencies being enabled.
			preview := evaluatePreview(t, version, source, &rules.ProvideMergeAllToArray)
			var got []string
			for _, diagnostic := range preview.Diagnostics {
				if !strings.Contains(diagnostic.Text, "Effect.provide accepts an array of layers and merges them itself") {
					t.Fatalf("unexpected diagnostic text %q", diagnostic.Text)
				}
				if span := preview.SourceText[diagnostic.Start:diagnostic.End]; span != "Layer.mergeAll(ALayer, BLayer)" {
					t.Fatalf("diagnostic span %q does not cover the Layer.mergeAll call", span)
				}
				lineStart := strings.LastIndex(preview.SourceText[:diagnostic.Start], "\n") + 1
				match := exportName.FindStringSubmatch(preview.SourceText[lineStart:diagnostic.Start])
				if match == nil {
					t.Fatalf("diagnostic at %d is not on an export line", diagnostic.Start)
				}
				got = append(got, match[1])
			}
			if !slices.Equal(got, want) {
				t.Fatalf("matched %v, want %v", got, want)
			}
		})
	}
}
