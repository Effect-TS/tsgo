package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/typescript-go/shim/ast"
)

func findFirstEffectGenCall(t *testing.T, tp *TypeParser, sf *ast.SourceFile) *EffectGenCallResult {
	t.Helper()
	var result *EffectGenCallResult
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if node == nil || result != nil {
			return
		}
		if parsed := tp.EffectGenCall(node); parsed != nil {
			result = parsed
			return
		}
		node.ForEachChild(func(child *ast.Node) bool {
			visit(child)
			return false
		})
	}
	visit(sf.AsNode())
	if result == nil {
		t.Fatal("expected to find Effect.gen call")
	}
	return result
}

func assertEffectGenFunctionReturnType(t *testing.T, version bundledeffect.EffectVersion, source string, want string) {
	t.Helper()
	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, source)
	defer done()

	parsed := findFirstEffectGenCall(t, tp, sf)
	if parsed.FunctionReturnType == nil {
		t.Fatal("expected FunctionReturnType")
	}
	if got := c.TypeToString(parsed.FunctionReturnType); got != want {
		t.Fatalf("FunctionReturnType = %q", got)
	}
}

func TestEffectGenCall_FunctionReturnType_NoPipe(t *testing.T) {
	t.Parallel()

	source := `
import { Effect } from "effect"

export const make = Effect.gen(function*() {
  return 1
})
`
	assertEffectGenFunctionReturnType(t, bundledeffect.EffectV4, source, "Effect<number, never, never>")
	assertEffectGenFunctionReturnType(t, bundledeffect.EffectV3, source, "Effect<number, never, never>")
}

func TestEffectGenCall_FunctionReturnType_GenericNoPipe(t *testing.T) {
	t.Parallel()

	source := `
import { Effect } from "effect"

export const make = <A>(a: A) => Effect.gen(function*() {
  return a
})
`
	assertEffectGenFunctionReturnType(t, bundledeffect.EffectV4, source, "Effect<A, never, never>")
	assertEffectGenFunctionReturnType(t, bundledeffect.EffectV3, source, "Effect<A, never, never>")
}

func TestEffectGenCall_FunctionReturnType_SelfObject(t *testing.T) {
	t.Parallel()

	source := `
import { Effect } from "effect"

const self = {
  prefix: "value"
}

export const make = Effect.gen({ self }, function*(this: typeof self) {
  return this.prefix
})
`
	assertEffectGenFunctionReturnType(t, bundledeffect.EffectV4, source, "Effect<string, never, never>")
}

func TestEffectGenCall_FunctionReturnType_GenericSelfObject(t *testing.T) {
	t.Parallel()

	sourceV4 := `
import { Effect } from "effect"

export const make = <A>(self: { readonly value: A }) => Effect.gen({ self }, function*(this: typeof self) {
  return this.value
})
`
	sourceV3 := `
import { Effect } from "effect"

export const make = <A>(self: { readonly value: A }) => Effect.gen(self, function*(this: typeof self) {
  return this.value
})
`
	assertEffectGenFunctionReturnType(t, bundledeffect.EffectV4, sourceV4, "Effect<A, never, never>")
	assertEffectGenFunctionReturnType(t, bundledeffect.EffectV3, sourceV3, "Effect<A, never, never>")
}
