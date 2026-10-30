package effecttest_test

import (
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestEffectSucceedWithUndefinedFixesCompile(t *testing.T) {
	t.Parallel()
	code := tsdiag.Effect_undefined_represents_the_same_outcome_as_Effect_succeed_undefined_or_Effect_succeed_void_0_effect_effectSucceedWithUndefined.Code()
	verifyFixesCompile(t, bundledeffect.EffectV4, "effectSucceedWithUndefined", code, func(title string) bool { return strings.HasPrefix(title, "Replace with Effect.undefined") }, []fixCompileCase{
		{fixture: "effectSucceedWithUndefined.ts", matches: 15, fixes: 15},
		{fixture: "effectSucceedWithUndefined_aliases.ts", matches: 12, fixes: 12},
		{fixture: "effectSucceedWithUndefined_preview.ts", matches: 1, fixes: 1},
		{fixture: "effectSucceedWithUndefined_namedImport.ts", matches: 3, fixes: 0},
		{fixture: "effectSucceedWithUndefined_scopedEffect.ts", matches: 4, fixes: 2},
		{fixture: "effectSucceedWithUndefined_callShapes.ts", matches: 5, fixes: 5},
		{fixture: "effectSucceedWithUndefined_namedUndefinedImport.ts", matches: 1, fixes: 1},
	})
}
