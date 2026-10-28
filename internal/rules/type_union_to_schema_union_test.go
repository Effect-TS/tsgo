package rules_test

import (
	"slices"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rules"
)

func TestTypeUnionToSchemaUnionMatches(t *testing.T) {
	t.Parallel()
	source := `import { Schema } from "effect"
export const A = Schema.Struct({ a: Schema.String })
export const B = Schema.Struct({ b: Schema.String })
export type TypeQueries = typeof A.Type | typeof B.Type
export type EncodedQueries = typeof A.Encoded | typeof B.Encoded
export type Nested = ((typeof A.Type) | (typeof B.Type | typeof A.Type))
export type TypeHelpers = Schema.Schema.Type<typeof A> | typeof B.Type
export type V3EncodedHelpers = Schema.Schema.Encoded<typeof A> | typeof B.Encoded
export type V4EncodedHelpers = Schema.Codec.Encoded<typeof A> | typeof B.Encoded
export type Late = typeof A.Type | typeof C.Type
export const C = Schema.Struct({ c: Schema.String })
export type Single = typeof A.Type
export type MixedSides = typeof A.Type | typeof B.Encoded
export type WithNull = typeof A.Type | typeof B.Type | null
export type ContextHelpers = Schema.Schema.Context<typeof A> | Schema.Schema.Context<typeof B>
export type Generic<_T> = typeof A.Type | typeof B.Type
declare const notSchema: { readonly Type: string; readonly Encoded: string }
export type NotSchema = typeof A.Type | typeof notSchema.Type
`
	sides := map[string]string{
		"TypeQueries":      "Type",
		"EncodedQueries":   "Encoded",
		"Nested":           "Type",
		"TypeHelpers":      "Type",
		"V3EncodedHelpers": "Encoded",
		"V4EncodedHelpers": "Encoded",
		"Late":             "Type",
	}

	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			want := []string{"TypeQueries", "EncodedQueries", "Nested", "TypeHelpers", "V3EncodedHelpers", "V4EncodedHelpers", "Late"}
			// Encoded helpers only exist as Schema.Schema.Encoded in v3 and as
			// Schema.Codec.Encoded in v4.
			unsupported := "V4EncodedHelpers"
			if version == bundledeffect.EffectV4 {
				unsupported = "V3EncodedHelpers"
			}
			want = slices.DeleteFunc(want, func(name string) bool { return name == unsupported })

			preview := evaluatePreview(t, version, source, &rules.TypeUnionToSchemaUnion)
			var got []string
			for _, diagnostic := range preview.Diagnostics {
				name := preview.SourceText[diagnostic.Start:diagnostic.End]
				side, ok := sides[name]
				if !ok {
					t.Fatalf("unexpected diagnostic on %q", name)
				}
				if !strings.Contains(diagnostic.Text, "unions Schema `."+side+"` types") {
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
