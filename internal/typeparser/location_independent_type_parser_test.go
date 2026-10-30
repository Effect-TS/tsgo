package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func TestParsersAcceptInstantiatedV3TypesWithoutLocation(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV3, "effect"); err != nil {
		t.Skip("Effect v3 not installed:", err)
	}

	for _, tc := range []struct {
		name  string
		decl  string
		parse func(*TypeParser, *checker.Type) []*checker.Type
		want  []string
	}{
		{"Effect", `import * as Effect from "effect/Effect"
declare const value: Effect.Effect<string, Error, number>`, func(tp *TypeParser, typ *checker.Type) []*checker.Type {
			if parsed := tp.EffectType(typ); parsed != nil {
				return []*checker.Type{parsed.a, parsed.e, parsed.r}
			}
			return nil
		}, []string{"string", "Error", "number"}},
		{"Layer", `import * as Layer from "effect/Layer"
declare const value: Layer.Layer<string, Error, number>`, func(tp *TypeParser, typ *checker.Type) []*checker.Type {
			if parsed := tp.LayerType(typ); parsed != nil {
				return []*checker.Type{parsed.rOut, parsed.e, parsed.rIn}
			}
			return nil
		}, []string{"string", "Error", "number"}},
		{"Schema", `import * as Schema from "effect/Schema"
declare const value: Schema.Schema<string, number>`, func(tp *TypeParser, typ *checker.Type) []*checker.Type {
			if parsed := tp.EffectSchemaTypes(typ); parsed != nil {
				return []*checker.Type{parsed.a, parsed.e}
			}
			return nil
		}, []string{"string", "number"}},
		{"Stream", `import * as Stream from "effect/Stream"
declare const value: Stream.Stream<string, Error, number>`, func(tp *TypeParser, typ *checker.Type) []*checker.Type {
			if parsed := tp.StreamType(typ); parsed != nil {
				return []*checker.Type{parsed.a, parsed.e, parsed.r}
			}
			return nil
		}, []string{"string", "Error", "number"}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV3Internal(t, tc.decl+"\nvalue\n")
			defer done()

			parts := tc.parse(tp, tp.GetTypeAtLocation(findIdentifierByText(t, sf, "value", 1)))
			if parts == nil {
				t.Fatalf("expected instantiated v3 %s type to parse without a location", tc.name)
			}
			for i, want := range tc.want {
				if got := c.TypeToString(parts[i]); got != want {
					t.Errorf("type argument %d: expected %s, got %s", i, want, got)
				}
			}
		})
	}
}
