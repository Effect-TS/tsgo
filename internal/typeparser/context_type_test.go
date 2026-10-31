package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
)

// Fixtures cover the Context types runProvidedContextToRunWith accepts; this
// covers the types the parser must reject.
func TestContextType_V4(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect v4 not installed:", err)
	}

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, `
import { Context, Effect, Layer } from "effect"

interface Config {
  readonly apiUrl: string
}

interface LocalContext<S> {
  readonly services: S
}

declare const untyped: any
declare const local: LocalContext<Config>

const anyRef = untyped
const effectRef = Effect.succeed(1)
const layerRef = Layer.empty
const localRef = local
`)
	defer done()

	for _, name := range []string{"anyRef", "effectRef", "layerRef", "localRef"} {
		if tp.ContextTypeAtLocation(findVariableInitializer(t, sf, name)) != nil {
			t.Errorf("expected %s not to parse as a Context type", name)
		}
	}
	if tp.ContextTypeAtLocation(nil) != nil {
		t.Error("expected a nil node not to parse as a Context type")
	}
	if tp.ContextType(nil) != nil {
		t.Error("expected a nil type not to parse as a Context type")
	}
}

func TestContextType_V3(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV3, "effect"); err != nil {
		t.Skip("Effect v3 not installed:", err)
	}

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV3Internal(t, `
import { Context } from "effect"

class Config extends Context.Tag("Config")<Config, { readonly apiUrl: string }>() {}

const made = Context.make(Config, { apiUrl: "https://api.example.com" })
const empty = Context.empty()
`)
	defer done()

	for _, name := range []string{"made", "empty"} {
		if tp.ContextTypeAtLocation(findVariableInitializer(t, sf, name)) != nil {
			t.Errorf("expected v3 %s not to parse as a Context type", name)
		}
	}
}
