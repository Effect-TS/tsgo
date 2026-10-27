package rules_test

import (
	"regexp"
	"slices"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

func TestMapConstToAsCallbackShapes(t *testing.T) {
	t.Parallel()
	source := `import { Effect } from "effect"
declare const source: Effect.Effect<number>
declare const ambient: string
declare let ambientLet: string
const enum Flag { On = 1 }
export const literal = source.pipe(Effect.map(() => "saved"))
export const negativeBigint = source.pipe(Effect.map(() => -1n))
export const constEnum = source.pipe(Effect.map(() => Flag.On))
export const ambientConst = source.pipe(Effect.map(() => ambient))
export const ambientMutable = source.pipe(Effect.map(() => ambientLet))
export const ambientLater = source.pipe(Effect.map(() => laterAmbient))
export const typeArguments = source.pipe(Effect.map<number, string>(() => "saved"))
export const restParameter = source.pipe(Effect.map((..._args: Array<number>) => "saved"))
export const asyncCallback = source.pipe(Effect.map(async () => "saved"))
export const notNegative = source.pipe(Effect.map(() => +1))
export const voidBody = source.pipe(Effect.map(() => undefined))
export const annotatedReturn = source.pipe(Effect.map((): "saved" => "saved"))
export const annotatedFunction = source.pipe(Effect.map(function(): number { return 1 }))
export const annotatedParameter = source.pipe(Effect.map((_: number) => "saved"))
declare const laterAmbient: string
`
	want := []string{"literal", "negativeBigint", "constEnum", "ambientConst", "annotatedParameter"}
	exportName := regexp.MustCompile(`export const (\w+)`)

	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			preview := evaluatePreview(t, version, source, &rules.MapConstToAs)
			var got []string
			for _, diagnostic := range preview.Diagnostics {
				if !strings.Contains(diagnostic.Text, "`Effect.as` expresses replacing the success value with a constant directly.") {
					t.Fatalf("unexpected diagnostic text %q", diagnostic.Text)
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
