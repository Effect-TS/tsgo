package effecttest_test

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestMapMapErrorToMapBothFixesCompile(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			pick := func(v3, v4 int) int {
				if version == bundledeffect.EffectV3 {
					return v3
				}
				return v4
			}
			verifyFixesCompile(t, version, "mapMapErrorToMapBoth",
				tsdiag.Effect_0_Effect_1_transform_the_success_and_failure_channels_one_step_at_a_time_Effect_2_expresses_both_transformations_in_a_single_step_effect_mapMapErrorToMapBoth.Code(),
				func(title string) bool {
					return title == "Replace with Effect.mapBoth" || title == "Replace with Effect.mapBothEager"
				},
				[]fixCompileCase{
					{fixture: "mapMapErrorToMapBoth.ts", matches: pick(22, 25), fixes: pick(21, 24)},
					{fixture: "mapMapErrorToMapBoth_preview.ts", matches: 1, fixes: 1},
					{fixture: "mapMapErrorToMapBoth_namedImportsNoTarget.ts", matches: 3, fixes: 0},
					{fixture: "mapMapErrorToMapBoth_namedImportsTarget.ts", matches: 1, fixes: 1},
					{fixture: "mapMapErrorToMapBoth_evaluationOrder.ts", matches: pick(2, 6), fixes: pick(2, 3)},
					{fixture: "mapMapErrorToMapBoth_crossPipe.ts", matches: pick(8, 11), fixes: pick(8, 9)},
					{fixture: "mapMapErrorToMapBoth_curried.ts", matches: pick(8, 10), fixes: pick(2, 3)},
					{fixture: "mapMapErrorToMapBoth_boundaries.ts", matches: pick(6, 9), fixes: pick(3, 4)},
					{fixture: "mapMapErrorToMapBoth_calleeExpressions.ts", matches: 4, fixes: 1},
					{fixture: "mapMapErrorToMapBoth_precedence.ts", matches: 8, fixes: 8},
					{fixture: "mapMapErrorToMapBoth_asi.ts", matches: 6, fixes: 3},
					{fixture: "mapMapErrorToMapBoth_exactFixes.ts", matches: 6, fixes: 6},
				})
		})
	}
}
