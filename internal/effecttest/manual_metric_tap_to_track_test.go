package effecttest_test

import (
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestManualMetricTapToTrackFixesCompile(t *testing.T) {
	t.Parallel()
	verifyFixesCompile(t, bundledeffect.EffectV4, "manualMetricTapToTrack",
		tsdiag.Effect_0_expresses_recording_a_metric_on_every_1_directly_effect_manualMetricTapToTrack.Code(),
		func(title string) bool { return strings.HasPrefix(title, "Replace with Effect.track") },
		[]fixCompileCase{
			{fixture: "manualMetricTapToTrack.ts", matches: 30, fixes: 30},
			{fixture: "manualMetricTapToTrack_preview.ts", matches: 1, fixes: 1},
			{fixture: "manualMetricTapToTrack_noTarget.ts", matches: 3, fixes: 0},
			{fixture: "manualMetricTapToTrack_bindings.ts", matches: 7, fixes: 6},
			{fixture: "manualMetricTapToTrack_localAliases.ts", matches: 3, fixes: 2},
			{fixture: "manualMetricTapToTrack_fixShapes.ts", matches: 12, fixes: 11},
		})
}
