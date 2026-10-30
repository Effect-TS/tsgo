package effecttest_test

import (
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestSuspendFailToFailSyncFixesCompile(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			cases := []fixCompileCase{
				{fixture: "suspendFailToFailSync.ts", matches: 25, fixes: 24},
				{fixture: "suspendFailToFailSync_shapes.ts", matches: 18, fixes: 17},
				{fixture: "suspendFailToFailSync_exactTypes.ts", matches: 6, fixes: 5},
			}
			if version == bundledeffect.EffectV4 {
				cases = []fixCompileCase{
					{fixture: "suspendFailToFailSync.ts", matches: 20, fixes: 19},
					{fixture: "suspendFailToFailSync_preview.ts", matches: 1, fixes: 1},
					{fixture: "suspendFailToFailSync_shapes.ts", matches: 15, fixes: 14},
					{fixture: "suspendFailToFailSync_exactTypes.ts", matches: 6, fixes: 5},
				}
			}
			verifyFixesCompile(t, version, "suspendFailToFailSync",
				tsdiag.Effect_0_builds_this_1_lazily_directly_instead_of_wrapping_Effect_2_in_Effect_suspend_effect_suspendFailToFailSync.Code(),
				func(title string) bool { return strings.HasPrefix(title, "Replace with Effect.") },
				cases)
		})
	}
}
