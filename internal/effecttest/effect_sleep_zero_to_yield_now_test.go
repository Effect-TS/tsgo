package effecttest_test

import (
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestEffectSleepZeroToYieldNowFixesCompile(t *testing.T) {
	t.Parallel()
	code := tsdiag.Consider_Effect_yieldNow_for_cooperative_yielding_instead_of_sleeping_for_zero_duration_effect_effectSleepZeroToYieldNow.Code()
	for version, mainMatches := range map[bundledeffect.EffectVersion]int{bundledeffect.EffectV3: 42, bundledeffect.EffectV4: 44} {
		verifyFixesCompile(t, version, "effectSleepZeroToYieldNow", code, func(title string) bool { return strings.HasPrefix(title, "Replace with Effect.yieldNow") }, []fixCompileCase{
			{fixture: "effectSleepZeroToYieldNow.ts", matches: mainMatches, fixes: mainMatches},
			{fixture: "effectSleepZeroToYieldNow_aliases.ts", matches: 15, fixes: 9},
			{fixture: "effectSleepZeroToYieldNow_preview.ts", matches: 1, fixes: 1},
		})
	}
}
