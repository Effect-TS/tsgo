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
