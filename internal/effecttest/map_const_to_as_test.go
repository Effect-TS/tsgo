package effecttest_test

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestMapConstToAsFixesCompile(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			verifyFixesCompile(t, version, "mapConstToAs",
				tsdiag.Effect_as_expresses_replacing_the_success_value_with_a_constant_directly_effect_mapConstToAs.Code(),
				func(title string) bool { return title == "Replace with Effect.as" },
				[]fixCompileCase{
					{fixture: "mapConstToAs.ts", matches: 23, fixes: 23},
					{fixture: "mapConstToAs_fixShapes.ts", matches: 4, fixes: 4},
				})
		})
	}
}
