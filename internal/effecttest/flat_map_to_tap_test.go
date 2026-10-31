package effecttest_test

import (
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestFlatMapToTapFixesCompile(t *testing.T) {
	t.Parallel()
	code := tsdiag.Effect_tap_expresses_this_observation_more_directly_than_Effect_0_with_a_manual_re_yield_effect_flatMapToTap.Code()
	for version, mainMatches := range map[bundledeffect.EffectVersion]int{bundledeffect.EffectV3: 23, bundledeffect.EffectV4: 23} {
		verifyFixesCompile(t, version, "flatMapToTap", code, func(title string) bool { return strings.HasPrefix(title, "Replace with Effect.tap") }, []fixCompileCase{
			{fixture: "flatMapToTap.ts", matches: mainMatches, fixes: mainMatches},
			{fixture: "flatMapToTap_aliases.ts", matches: 6, fixes: 6},
			{fixture: "flatMapToTap_preview.ts", matches: 1, fixes: 1},
			{fixture: "flatMapToTap_noTarget.ts", matches: 1, fixes: 0},
			{fixture: "flatMapToTap_comments.ts", matches: 2, fixes: 2},
		})
	}
}
