// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use libtest_mimic::Trial;

use crate::fixes_compile_test::{FixCompileCase, verify_fixes_compile};
use crate::run_case;

// go test: internal/effecttest/flat_map_identity_to_flatten_test.go:11 TestFlatMapIdentityToFlattenFixesCompile f75097bef80a6d08
fn test_flat_map_identity_to_flatten_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let code = tsdiag::EFFECT_FLATTEN_EXPRESSES_THIS_UNWRAPPING_MORE_DIRECTLY_THAN_EFFECT_FLAT_MAP_WITH_AN_IDENTITY_CALLBACK_EFFECT_FLAT_MAP_IDENTITY_TO_FLATTEN.code();
    verify_fixes_compile(
        t,
        EffectVersion::V3,
        "flatMapIdentityToFlatten",
        code,
        |title| title.starts_with("Replace with Effect.flatten"),
        &[FixCompileCase { fixture: "flatMapIdentityToFlatten.ts", matches: 19, fixes: 19 }],
    );
    verify_fixes_compile(
        t,
        EffectVersion::V4,
        "flatMapIdentityToFlatten",
        code,
        |title| title.starts_with("Replace with Effect.flatten"),
        &[
            FixCompileCase { fixture: "flatMapIdentityToFlatten.ts", matches: 18, fixes: 18 },
            FixCompileCase { fixture: "flatMapIdentityToFlatten_preview.ts", matches: 1, fixes: 1 },
        ],
    );
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestFlatMapIdentityToFlattenFixesCompile".to_string(),
        test_flat_map_identity_to_flatten_fixes_compile,
    )]
}
