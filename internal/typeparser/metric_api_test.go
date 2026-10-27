package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
)

func TestIsNodeReferenceToEffectMetricModuleApi(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			source := `
				import { Effect, Metric } from "effect"
				import { update } from "effect/Metric"
				import * as M from "effect/Metric"
				const fake = { update: (_metric: unknown, _input: number) => Effect.void }
				const updateAlias = Metric.update
				const root = () => Metric.update
				const named = () => update
				const namespace = () => M.update
				const viaAlias = () => updateAlias
				const modify = () => Metric.modify
				const lookalike = () => fake.update
				const effectTap = () => Effect.tap
			`
			cases := []struct {
				name   string
				member string
				want   bool
			}{
				{name: "root", member: "update", want: true},
				{name: "named", member: "update", want: true},
				{name: "namespace", member: "update", want: true},
				{name: "viaAlias", member: "update", want: true},
				{name: "modify", member: "update"},
				{name: "modify", member: "modify", want: true},
				{name: "lookalike", member: "update"},
				{name: "effectTap", member: "update"},
				{name: "effectTap", member: "tap"},
			}
			_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, version, source)
			defer done()
			for _, tt := range cases {
				function := findReturningDispatchTestFunction(t, sf, tt.name)
				if got := tp.IsNodeReferenceToEffectMetricModuleApi(GetFunctionLikeBody(function), tt.member); got != tt.want {
					t.Errorf("%s as %s = %v, want %v", tt.name, tt.member, got, tt.want)
				}
			}
		})
	}
}
