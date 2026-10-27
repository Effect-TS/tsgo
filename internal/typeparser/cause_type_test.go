package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
)

func TestIsNodeReferenceToEffectCauseModuleApi(t *testing.T) {
	t.Parallel()
	type causeReferenceCase struct {
		name   string
		member string
		want   bool
	}
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			source := `
				import { Cause, Exit } from "effect"
				import * as C from "effect/Cause"
				import { die as causeDie, fail as causeFail } from "effect/Cause"
				const Local = { Cause: { fail: <E>(e: E) => e, die: (d: unknown) => d } }
				const failAlias = Cause.fail
				const namespace = () => Cause.fail
				const subpath = () => C.die
				const named = () => causeFail
				const namedDie = () => causeDie
				const alias = () => failAlias
				const empty = () => Cause.empty
				const lookalike = () => Local.Cause.fail
				const otherModule = () => Exit.fail
			`
			cases := []causeReferenceCase{
				{name: "namespace", member: "fail", want: true},
				{name: "namespace", member: "die"},
				{name: "subpath", member: "die", want: true},
				{name: "named", member: "fail", want: true},
				{name: "namedDie", member: "die", want: true},
				{name: "alias", member: "fail", want: true},
				{name: "empty", member: "empty", want: true},
				{name: "lookalike", member: "fail"},
				{name: "otherModule", member: "fail"},
			}
			if version == bundledeffect.EffectV4 {
				source += `
				const noSuchElement = () => Cause.NoSuchElementError
			`
				cases = append(cases, causeReferenceCase{name: "noSuchElement", member: "NoSuchElementError", want: true})
			}
			_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, source)
			defer done()
			for _, tt := range cases {
				function := findReturningDispatchTestFunction(t, sf, tt.name)
				if got := tp.IsNodeReferenceToEffectCauseModuleApi(GetFunctionLikeBody(function), tt.member); got != tt.want {
					t.Errorf("%s as %s = %v, want %v", tt.name, tt.member, got, tt.want)
				}
			}
		})
	}
}

func TestEffectModuleIdentifierAt(t *testing.T) {
	t.Parallel()
	cases := []struct {
		name   string
		source string
		want   string
	}{
		{name: "barrel", source: `import { Effect } from "effect"
const target = () => 1`, want: "Effect"},
		{name: "aliasedBarrel", source: `import { Effect as Fx } from "effect"
const target = () => 1`, want: "Fx"},
		{name: "namespace", source: `import * as Fx from "effect/Effect"
const target = () => 1`, want: "Fx"},
		{name: "namedImportsOnly", source: `import { fail } from "effect/Effect"
const target = () => fail`},
		{name: "shadowed", source: `import { Effect } from "effect"
const outer = (Effect: string) => {
  const target = () => Effect
  return target
}`},
		{name: "unrelatedEffect", source: `const Effect = { fail: (e: unknown) => e }
const target = () => Effect`},
		{name: "typeOnlyImport", source: `import type { Effect } from "effect"
const target = () => 1`},
		{name: "typeOnlySpecifier", source: `import { type Effect } from "effect"
const target = () => 1`},
		{name: "typeOnlyNamespace", source: `import type * as Fx from "effect/Effect"
const target = () => 1`},
	}
	for _, tt := range cases {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, bundledeffect.EffectV4, tt.source)
			defer done()
			body := GetFunctionLikeBody(findReturningDispatchTestFunction(t, sf, "target"))
			if got := tp.EffectModuleIdentifierAt(body); got != tt.want {
				t.Fatalf("EffectModuleIdentifierAt = %q, want %q", got, tt.want)
			}
		})
	}
}
