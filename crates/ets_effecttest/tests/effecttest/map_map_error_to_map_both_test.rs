// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use libtest_mimic::Trial;

use crate::fixes_compile_test::{FixCompileCase, verify_fixes_compile};
use crate::run_case;

// go test: internal/effecttest/map_map_error_to_map_both_test.go:10 TestMapMapErrorToMapBothFixesCompile 0c8741f208c70391
fn test_map_map_error_to_map_both_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    for version in [EffectVersion::V3, EffectVersion::V4] {
        t.run(version.as_str(), |t| {
            t.parallel();
            let pick = |v3: usize, v4: usize| -> usize {
                if version == EffectVersion::V3 {
                    return v3;
                }
                v4
            };
            verify_fixes_compile(
                t,
                version,
                "mapMapErrorToMapBoth",
                tsdiag::EFFECT_0_EFFECT_1_TRANSFORM_THE_SUCCESS_AND_FAILURE_CHANNELS_ONE_STEP_AT_A_TIME_EFFECT_2_EXPRESSES_BOTH_TRANSFORMATIONS_IN_A_SINGLE_STEP_EFFECT_MAP_MAP_ERROR_TO_MAP_BOTH.code(),
                |title| title == "Replace with Effect.mapBoth" || title == "Replace with Effect.mapBothEager",
                &[
                    FixCompileCase { fixture: "mapMapErrorToMapBoth.ts", matches: pick(22, 25), fixes: pick(21, 24) },
                    FixCompileCase { fixture: "mapMapErrorToMapBoth_preview.ts", matches: 1, fixes: 1 },
                    FixCompileCase { fixture: "mapMapErrorToMapBoth_namedImportsNoTarget.ts", matches: 3, fixes: 0 },
                    FixCompileCase { fixture: "mapMapErrorToMapBoth_namedImportsTarget.ts", matches: 1, fixes: 1 },
                    FixCompileCase {
                        fixture: "mapMapErrorToMapBoth_evaluationOrder.ts",
                        matches: pick(2, 6),
                        fixes: pick(2, 3),
                    },
                    FixCompileCase { fixture: "mapMapErrorToMapBoth_crossPipe.ts", matches: pick(8, 11), fixes: pick(8, 9) },
                    FixCompileCase { fixture: "mapMapErrorToMapBoth_curried.ts", matches: pick(8, 10), fixes: pick(2, 3) },
                    FixCompileCase { fixture: "mapMapErrorToMapBoth_boundaries.ts", matches: pick(6, 9), fixes: pick(3, 4) },
                    FixCompileCase { fixture: "mapMapErrorToMapBoth_calleeExpressions.ts", matches: 4, fixes: 1 },
                    FixCompileCase { fixture: "mapMapErrorToMapBoth_precedence.ts", matches: 8, fixes: 8 },
                    FixCompileCase { fixture: "mapMapErrorToMapBoth_asi.ts", matches: 6, fixes: 3 },
                    FixCompileCase { fixture: "mapMapErrorToMapBoth_exactFixes.ts", matches: 6, fixes: 6 },
                ],
            );
        });
    }
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestMapMapErrorToMapBothFixesCompile".to_string(),
        test_map_map_error_to_map_both_fixes_compile,
    )]
}
