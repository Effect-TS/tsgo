// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use libtest_mimic::Trial;

use crate::fixes_compile_test::{FixCompileCase, verify_fixes_compile};
use crate::run_case;

// go test: internal/effecttest/type_union_to_schema_union_test.go:10 TestTypeUnionToSchemaUnionFixesCompile c3fb7c0299665180
fn test_type_union_to_schema_union_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let code = tsdiag::THIS_TYPE_ALIAS_UNIONS_SCHEMA_0_TYPES_WITHOUT_A_RUNTIME_SCHEMA_COMPOSE_THEM_WITH_SCHEMA_UNION_AND_DERIVE_THE_TYPE_FROM_IT_EFFECT_TYPE_UNION_TO_SCHEMA_UNION.code();
    for (version, aliased_matches) in [(EffectVersion::V3, 3), (EffectVersion::V4, 4)] {
        verify_fixes_compile(
            t,
            version,
            "typeUnionToSchemaUnion",
            code,
            |title| title == "Convert to Schema.Union",
            &[
                FixCompileCase { fixture: "typeUnionToSchemaUnion.ts", matches: 25, fixes: 22 },
                FixCompileCase {
                    fixture: "typeUnionToSchemaUnion_aliasedImport.ts",
                    matches: aliased_matches,
                    fixes: aliased_matches,
                },
                FixCompileCase {
                    fixture: "typeUnionToSchemaUnion_preview.ts",
                    matches: 1,
                    fixes: 1,
                },
            ],
        );
    }
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestTypeUnionToSchemaUnionFixesCompile".to_string(),
        test_type_union_to_schema_union_fixes_compile,
    )]
}
