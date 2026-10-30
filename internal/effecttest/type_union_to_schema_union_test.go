package effecttest_test

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func TestTypeUnionToSchemaUnionFixesCompile(t *testing.T) {
	t.Parallel()
	code := tsdiag.This_type_alias_unions_Schema_0_types_without_a_runtime_schema_compose_them_with_Schema_Union_and_derive_the_type_from_it_effect_typeUnionToSchemaUnion.Code()
	for version, aliasedMatches := range map[bundledeffect.EffectVersion]int{bundledeffect.EffectV3: 3, bundledeffect.EffectV4: 4} {
		verifyFixesCompile(t, version, "typeUnionToSchemaUnion", code,
			func(title string) bool { return title == "Convert to Schema.Union" },
			[]fixCompileCase{
				{fixture: "typeUnionToSchemaUnion.ts", matches: 25, fixes: 22},
				{fixture: "typeUnionToSchemaUnion_aliasedImport.ts", matches: aliasedMatches, fixes: aliasedMatches},
				{fixture: "typeUnionToSchemaUnion_preview.ts", matches: 1, fixes: 1},
			})
	}
}
