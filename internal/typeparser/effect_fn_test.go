package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/typescript-go/shim/ast"
)

func findFirstEffectFnCall(t *testing.T, tp *TypeParser, sf *ast.SourceFile) *EffectFnCallResult {
	t.Helper()
	var result *EffectFnCallResult
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if node == nil || result != nil {
			return
		}
		if parsed := tp.EffectFnCall(node); parsed != nil {
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
		t.Fatal("expected to find Effect.fn call")
	}
	return result
}

func compileEffectFnTest(t *testing.T, version bundledeffect.EffectVersion, source string) (*TypeParser, *ast.SourceFile, func()) {
	t.Helper()
	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, source)
	return tp, sf, done
}

func assertEffectFnFunctionReturnType(t *testing.T, version bundledeffect.EffectVersion, source string, want string) {
	t.Helper()
	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, source)
	defer done()

	parsed := findFirstEffectFnCall(t, tp, sf)
	if parsed.FunctionReturnType == nil {
		t.Fatal("expected FunctionReturnType")
	}
	if got := c.TypeToString(parsed.FunctionReturnType); got != want {
		t.Fatalf("FunctionReturnType = %q", got)
	}
}

func TestEffectFnCall_FunctionReturnType_NoPipe(t *testing.T) {
	t.Parallel()

	source := `
import { Effect } from "effect"

export const make = Effect.fn(function*(n: number) {
  return n + 1
})
`
	assertEffectFnFunctionReturnType(t, bundledeffect.EffectV4, source, "Effect<number, never, never>")
	assertEffectFnFunctionReturnType(t, bundledeffect.EffectV3, source, "Effect<number, never, never>")
}

func TestEffectFnCall_FunctionReturnType_FirstPipeInput(t *testing.T) {
	t.Parallel()

	source := `
import { Effect } from "effect"

export const make = Effect.fn(
  function*(n: number) {
    return n + 1
  },
  Effect.map((n) => n.toString())
)
`
	assertEffectFnFunctionReturnType(t, bundledeffect.EffectV4, source, "Effect<number, never, never>")
	assertEffectFnFunctionReturnType(t, bundledeffect.EffectV3, source, "Effect<number, never, never>")
}

func TestEffectFnCall_FunctionReturnType_Generic(t *testing.T) {
	t.Parallel()

	source := `
import { Effect } from "effect"

export const make = Effect.fn(
  function*<A>(a: A) {
    return a
  },
  Effect.map((a) => [a] as const)
)
`
	assertEffectFnFunctionReturnType(t, bundledeffect.EffectV4, source, "Effect<A, never, never>")
	assertEffectFnFunctionReturnType(t, bundledeffect.EffectV3, source, "Effect<A, never, never>")
}

func TestEffectFnCall_FunctionReturnType_GenericNoPipe(t *testing.T) {
	t.Parallel()

	source := `
import { Effect } from "effect"

export const make = Effect.fn(function*<A>(a: A) {
  return a
})
`
	assertEffectFnFunctionReturnType(t, bundledeffect.EffectV4, source, "Effect<A, never, never>")
	assertEffectFnFunctionReturnType(t, bundledeffect.EffectV3, source, "Effect<A, never, never>")
}

func TestEffectFnCall_FunctionReturnType_ConstrainedGeneric(t *testing.T) {
	t.Parallel()

	source := `
import { Effect } from "effect"

export const make = Effect.fn(
  function*<A extends { readonly id: string }>(a: A) {
    return a
  },
  Effect.tap((a) => Effect.log(a.id))
)
`
	assertEffectFnFunctionReturnType(t, bundledeffect.EffectV4, source, "Effect<A, never, never>")
	assertEffectFnFunctionReturnType(t, bundledeffect.EffectV3, source, "Effect<A, never, never>")
}

func TestEffectFnCall_FunctionReturnType_NamedSpan(t *testing.T) {
	t.Parallel()

	source := `
import { Effect } from "effect"

export const make = Effect.fn(
  "named-span"
)(
  function*(n: number) {
    return n + 1
  },
  Effect.map((n) => n.toString())
)
`
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV4, bundledeffect.EffectV3} {
		c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, source)
		parsed := findFirstEffectFnCall(t, tp, sf)
		if parsed.TraceExpression == nil {
			done()
			t.Fatal("expected TraceExpression")
		}
		if parsed.FunctionReturnType == nil {
			done()
			t.Fatal("expected FunctionReturnType")
		}
		if got := c.TypeToString(parsed.FunctionReturnType); got != "Effect<number, never, never>" {
			done()
			t.Fatalf("FunctionReturnType = %q", got)
		}
		done()
	}
}

func TestEffectFnCall_FunctionReturnType_SelfObject(t *testing.T) {
	t.Parallel()

	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, `
import { Effect } from "effect"

const self = {
  prefix: "value"
}

export const make = Effect.fn(
  { self },
  function*(this: typeof self, n: number) {
    return this.prefix + n.toString()
  },
  Effect.map((value) => value.length)
)
`)
	defer done()

	parsed := findFirstEffectFnCall(t, tp, sf)
	if parsed.OptionsNode == nil {
		t.Fatal("expected OptionsNode")
	}
	if parsed.FunctionReturnType == nil {
		t.Fatal("expected FunctionReturnType")
	}
	if got := c.TypeToString(parsed.FunctionReturnType); got != "Effect<string, never, never>" {
		t.Fatalf("FunctionReturnType = %q", got)
	}
}
