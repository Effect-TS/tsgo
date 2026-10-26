package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
)

type functionReferenceCase struct {
	name   string
	member string
	want   bool
}

func TestIsNodeReferenceToEffectFunctionModuleApi(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			source := `
				import { Channel, Function, identity } from "effect"
				import { identity as id } from "effect/Function"
				import * as F from "effect/Function"
				const localIdentity = <A>(a: A): A => a
				const fake = { identity: <A>(a: A): A => a }
				const root = () => identity
				const aliased = () => id
				const namespace = () => Function.identity
				const subpath = () => F.identity
				const otherExport = () => F.pipe
				const channel = () => Channel.identity
				const local = () => localIdentity
				const lookalike = () => fake.identity
			`
			cases := []functionReferenceCase{
				{name: "root", member: "identity", want: true},
				{name: "aliased", member: "identity", want: true},
				{name: "namespace", member: "identity", want: true},
				{name: "subpath", member: "identity", want: true},
				{name: "otherExport", member: "identity"},
				{name: "otherExport", member: "pipe", want: true},
				{name: "channel", member: "identity"},
				{name: "local", member: "identity"},
				{name: "lookalike", member: "identity"},
			}
			if version == bundledeffect.EffectV3 {
				source += `
				import { Schedule, Stream } from "effect"
				const stream = () => Stream.identity
				const schedule = () => Schedule.identity
			`
				cases = append(cases,
					functionReferenceCase{name: "stream", member: "identity"},
					functionReferenceCase{name: "schedule", member: "identity"},
				)
			}
			_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, source)
			defer done()
			for _, tt := range cases {
				function := findReturningDispatchTestFunction(t, sf, tt.name)
				if got := tp.IsNodeReferenceToEffectFunctionModuleApi(GetFunctionLikeBody(function), tt.member); got != tt.want {
					t.Errorf("%s as %s = %v, want %v", tt.name, tt.member, got, tt.want)
				}
			}
		})
	}
}
