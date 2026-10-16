package typeparser

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func TestStrictEffectTypeCachesOnlyEffectNamedTypes(t *testing.T) {
	t.Parallel()

	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, `import { Effect, Layer, Stream } from "effect"
export const effect = Effect.succeed(1)
export const stream = Stream.make(1)
export const layer = Layer.empty
export const number = 1
export const object = { value: 1 }
`)
	defer done()

	typeOf := func(name string) *checker.Type {
		typ := tp.GetTypeAtLocation(findIdentifierByText(t, sf, name, 0))
		if typ == nil {
			t.Fatalf("expected a type for %s", name)
		}
		return typ
	}

	for _, name := range []string{"stream", "layer", "number", "object"} {
		typ := typeOf(name)
		for range 2 {
			if tp.StrictEffectType(typ) != nil {
				t.Errorf("%s: expected no strict Effect type", name)
			}
		}
		if tp.links.StrictEffectType.TryGet(typ) != nil {
			t.Errorf("%s: rejected type has a cache entry", name)
		}
	}
	typ := typeOf("effect")
	effect := tp.StrictEffectType(typ)
	if effect == nil {
		t.Fatal("effect: expected a strict Effect type")
	}
	if got := c.TypeToString(effect.A()); got != "number" {
		t.Errorf("effect: success type %q, want number", got)
	}
	if entry := tp.links.StrictEffectType.TryGet(typ); entry == nil || *entry != effect {
		t.Fatal("effect: expected a cache entry holding the result")
	}
	if tp.StrictEffectType(typ) != effect {
		t.Error("effect: expected the cached result on a repeated query")
	}
}
