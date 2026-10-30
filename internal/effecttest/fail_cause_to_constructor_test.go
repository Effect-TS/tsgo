package effecttest_test

import (
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestFailCauseToConstructorFixesCompile(t *testing.T) {
	t.Parallel()
	code := tsdiag.Effect_0_expresses_this_failure_directly_instead_of_wrapping_Cause_2_in_Effect_1_effect_failCauseToConstructor.Code()
	verifyFixesCompile(t, bundledeffect.EffectV3, "failCauseToConstructor", code, func(title string) bool { return strings.HasPrefix(title, "Replace with Effect.") }, []fixCompileCase{
		{fixture: "failCauseToConstructor.ts", matches: 24, fixes: 21},
		{fixture: "failCauseToConstructor_namedImports.ts", matches: 3, fixes: 0},
		{fixture: "failCauseToConstructor_aliasedEffect.ts", matches: 2, fixes: 2},
		{fixture: "failCauseToConstructor_lazyObjectLiteral.ts", matches: 1, fixes: 1},
	})
	verifyFixesCompile(t, bundledeffect.EffectV4, "failCauseToConstructor", code, func(title string) bool { return strings.HasPrefix(title, "Replace with Effect.") }, []fixCompileCase{
		{fixture: "failCauseToConstructor.ts", matches: 24, fixes: 22},
		{fixture: "failCauseToConstructor_preview.ts", matches: 1, fixes: 1},
		{fixture: "failCauseToConstructor_namedImports.ts", matches: 3, fixes: 0},
		{fixture: "failCauseToConstructor_aliasedEffect.ts", matches: 2, fixes: 2},
		{fixture: "failCauseToConstructor_lazyObjectLiteral.ts", matches: 1, fixes: 1},
	})
}
