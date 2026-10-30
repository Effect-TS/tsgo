package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
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

func TestEffectFnCall_ConstantAlias(t *testing.T) {
	t.Parallel()

	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, `
import { Effect } from "effect"

const fn = Effect.fn
export const make = fn(function*() {
  return 1
})
`)
		parsed := findFirstEffectFnCall(t, tp, sf)
		if parsed.effectModule != nil {
			done()
			t.Fatal("expected a constant fn alias to have no directly replaceable Effect module receiver")
		}
		done()
	}
}

func TestEffectFnCall_FunctionReturnType(t *testing.T) {
	t.Parallel()

	bothVersions := []bundledeffect.EffectVersion{bundledeffect.EffectV4, bundledeffect.EffectV3}
	tests := []struct {
		name        string
		versions    []bundledeffect.EffectVersion
		source      string
		want        string
		wantTrace   bool
		wantOptions bool
	}{
		{
			name:     "NoPipe",
			versions: bothVersions,
			source: `
import { Effect } from "effect"

export const make = Effect.fn(function*(n: number) {
  return n + 1
})
`,
			want: "Effect<number, never, never>",
		},
		{
			name:     "FirstPipeInput",
			versions: bothVersions,
			source: `
import { Effect } from "effect"

export const make = Effect.fn(
  function*(n: number) {
    return n + 1
  },
  Effect.map((n) => n.toString())
)
`,
			want: "Effect<number, never, never>",
		},
		{
			name:     "Generic",
			versions: bothVersions,
			source: `
import { Effect } from "effect"

export const make = Effect.fn(
  function*<A>(a: A) {
    return a
  },
  Effect.map((a) => [a] as const)
)
`,
			want: "Effect<A, never, never>",
		},
		{
			name:     "GenericNoPipe",
			versions: bothVersions,
			source: `
import { Effect } from "effect"

export const make = Effect.fn(function*<A>(a: A) {
  return a
})
`,
			want: "Effect<A, never, never>",
		},
		{
			name:     "ConstrainedGeneric",
			versions: bothVersions,
			source: `
import { Effect } from "effect"

export const make = Effect.fn(
  function*<A extends { readonly id: string }>(a: A) {
    return a
  },
  Effect.tap((a) => Effect.log(a.id))
)
`,
			want: "Effect<A, never, never>",
		},
		{
			name:     "NamedSpan",
			versions: bothVersions,
			source: `
import { Effect } from "effect"

export const make = Effect.fn(
  "named-span"
)(
  function*(n: number) {
    return n + 1
  },
  Effect.map((n) => n.toString())
)
`,
			want:      "Effect<number, never, never>",
			wantTrace: true,
		},
		{
			name:     "SelfObject",
			versions: []bundledeffect.EffectVersion{bundledeffect.EffectV4},
			source: `
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
`,
			want:        "Effect<string, never, never>",
			wantOptions: true,
		},
	}

	for _, tt := range tests {
		for _, version := range tt.versions {
			c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, tt.source)
			parsed := findFirstEffectFnCall(t, tp, sf)
			if got := parsed.traceExpression != nil; got != tt.wantTrace {
				t.Errorf("%s/%s: has trace expression = %v, want %v", tt.name, version, got, tt.wantTrace)
			}
			if got := parsed.optionsNode != nil; got != tt.wantOptions {
				t.Errorf("%s/%s: has options node = %v, want %v", tt.name, version, got, tt.wantOptions)
			}
			if parsed.functionReturnType == nil {
				t.Errorf("%s/%s: expected FunctionReturnType", tt.name, version)
			} else if got := c.TypeToString(parsed.FunctionReturnType()); got != tt.want {
				t.Errorf("%s/%s: FunctionReturnType = %q, want %q", tt.name, version, got, tt.want)
			}
			done()
		}
	}
}
