// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use libtest_mimic::Trial;

use crate::fixes_compile_test::{FixCompileCase, verify_fixes_compile};
use crate::run_case;

// go test: internal/effecttest/provide_merge_all_to_array_test.go:10 TestProvideMergeAllToArrayFixesCompile 9c8ce94dfc7453db
fn test_provide_merge_all_to_array_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    for version in [EffectVersion::V3, EffectVersion::V4] {
        t.run(version.as_str(), |t| {
            t.parallel();
            let main_matches = match version {
                EffectVersion::V3 => 17,
                EffectVersion::V4 => 19,
            };
            verify_fixes_compile(
                t,
                version,
                "provideMergeAllToArray",
                tsdiag::EFFECT_PROVIDE_ACCEPTS_AN_ARRAY_OF_LAYERS_AND_MERGES_THEM_ITSELF_PASS_THE_LAYERS_AS_AN_ARRAY_INSTEAD_OF_WRAPPING_THEM_IN_LAYER_MERGE_ALL_EFFECT_PROVIDE_MERGE_ALL_TO_ARRAY.code(),
                |title| title == "Pass the layers as an array to Effect.provide",
                &[
                    FixCompileCase { fixture: "provideMergeAllToArray.ts", matches: main_matches, fixes: main_matches },
                    FixCompileCase { fixture: "provideMergeAllToArray_preview.ts", matches: 1, fixes: 1 },
                    FixCompileCase { fixture: "provideMergeAllToArray_shapes.ts", matches: 3, fixes: 3 },
                ],
            );
        });
    }
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestProvideMergeAllToArrayFixesCompile".to_string(),
        test_provide_merge_all_to_array_fixes_compile,
    )]
}
