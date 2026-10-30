package effecttest_test

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestProvideMergeAllToArrayFixesCompile(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			mainMatches := map[bundledeffect.EffectVersion]int{bundledeffect.EffectV3: 17, bundledeffect.EffectV4: 19}[version]
			verifyFixesCompile(t, version, "provideMergeAllToArray",
				tsdiag.Effect_provide_accepts_an_array_of_layers_and_merges_them_itself_pass_the_layers_as_an_array_instead_of_wrapping_them_in_Layer_mergeAll_effect_provideMergeAllToArray.Code(),
				func(title string) bool { return title == "Pass the layers as an array to Effect.provide" },
				[]fixCompileCase{
					{fixture: "provideMergeAllToArray.ts", matches: mainMatches, fixes: mainMatches},
					{fixture: "provideMergeAllToArray_preview.ts", matches: 1, fixes: 1},
					{fixture: "provideMergeAllToArray_shapes.ts", matches: 3, fixes: 3},
				})
		})
	}
}
