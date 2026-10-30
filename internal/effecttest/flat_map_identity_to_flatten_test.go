package effecttest_test

import (
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestFlatMapIdentityToFlattenFixesCompile(t *testing.T) {
	t.Parallel()
	code := tsdiag.Effect_flatten_expresses_this_unwrapping_more_directly_than_Effect_flatMap_with_an_identity_callback_effect_flatMapIdentityToFlatten.Code()
	verifyFixesCompile(t, bundledeffect.EffectV3, "flatMapIdentityToFlatten", code, func(title string) bool { return strings.HasPrefix(title, "Replace with Effect.flatten") }, []fixCompileCase{
		{fixture: "flatMapIdentityToFlatten.ts", matches: 19, fixes: 19},
	})
	verifyFixesCompile(t, bundledeffect.EffectV4, "flatMapIdentityToFlatten", code, func(title string) bool { return strings.HasPrefix(title, "Replace with Effect.flatten") }, []fixCompileCase{
		{fixture: "flatMapIdentityToFlatten.ts", matches: 18, fixes: 18},
		{fixture: "flatMapIdentityToFlatten_preview.ts", matches: 1, fixes: 1},
	})
}
