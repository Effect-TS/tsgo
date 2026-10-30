package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
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

func TestEffectGenCall_FunctionReturnType(t *testing.T) {
	t.Parallel()

	bothVersions := []bundledeffect.EffectVersion{bundledeffect.EffectV4, bundledeffect.EffectV3}
	tests := []struct {
		name     string
		versions []bundledeffect.EffectVersion
		source   string
		want     string
	}{
		{
			name:     "NoPipe",
			versions: bothVersions,
			source: `
import { Effect } from "effect"

export const make = Effect.gen(function*() {
  return 1
})
`,
			want: "Effect<number, never, never>",
		},
		{
			name:     "GenericNoPipe",
			versions: bothVersions,
			source: `
import { Effect } from "effect"

export const make = <A>(a: A) => Effect.gen(function*() {
  return a
})
`,
			want: "Effect<A, never, never>",
		},
		{
			name:     "SelfObject",
			versions: []bundledeffect.EffectVersion{bundledeffect.EffectV4},
			source: `
import { Effect } from "effect"

const self = {
  prefix: "value"
}

export const make = Effect.gen({ self }, function*(this: typeof self) {
  return this.prefix
})
`,
			want: "Effect<string, never, never>",
		},
		{
			name:     "GenericSelfObject",
			versions: []bundledeffect.EffectVersion{bundledeffect.EffectV4},
			source: `
import { Effect } from "effect"

export const make = <A>(self: { readonly value: A }) => Effect.gen({ self }, function*(this: typeof self) {
  return this.value
})
`,
			want: "Effect<A, never, never>",
		},
		{
			name:     "GenericSelfArgument",
			versions: []bundledeffect.EffectVersion{bundledeffect.EffectV3},
			source: `
import { Effect } from "effect"

export const make = <A>(self: { readonly value: A }) => Effect.gen(self, function*(this: typeof self) {
  return this.value
})
`,
			want: "Effect<A, never, never>",
		},
	}

	for _, tt := range tests {
		for _, version := range tt.versions {
			c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, tt.source)
			parsed := findFirstEffectGenCall(t, tp, sf)
			returnType := tp.GetTypeAtLocation(parsed.call.AsNode())
			if returnType == nil {
				t.Errorf("%s/%s: expected return type", tt.name, version)
			} else if got := c.TypeToString(returnType); got != tt.want {
				t.Errorf("%s/%s: return type = %q, want %q", tt.name, version, got, tt.want)
			}
			done()
		}
	}
}

func TestEffectGenCall_ConstantAlias(t *testing.T) {
	t.Parallel()

	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, `
import { Effect } from "effect"

const gen = Effect.gen
export const make = gen(function*() {
  return 1
})
`)
		parsed := findFirstEffectGenCall(t, tp, sf)
		if parsed.effectModule != nil {
			done()
			t.Fatal("expected a constant gen alias to have no directly replaceable Effect module receiver")
		}
		done()
	}
}
