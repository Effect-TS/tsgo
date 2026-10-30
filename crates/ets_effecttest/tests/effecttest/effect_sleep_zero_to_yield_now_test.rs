// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use libtest_mimic::Trial;

use crate::fixes_compile_test::{FixCompileCase, verify_fixes_compile};
use crate::run_case;

// go test: internal/effecttest/effect_sleep_zero_to_yield_now_test.go:11 TestEffectSleepZeroToYieldNowFixesCompile 9b59b22077bcc99a
fn test_effect_sleep_zero_to_yield_now_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let code = tsdiag::CONSIDER_EFFECT_YIELD_NOW_FOR_COOPERATIVE_YIELDING_INSTEAD_OF_SLEEPING_FOR_ZERO_DURATION_EFFECT_EFFECT_SLEEP_ZERO_TO_YIELD_NOW.code();
    for (version, main_matches) in [(EffectVersion::V3, 42), (EffectVersion::V4, 44)] {
        verify_fixes_compile(
            t,
            version,
            "effectSleepZeroToYieldNow",
            code,
            |title| title.starts_with("Replace with Effect.yieldNow"),
            &[
                FixCompileCase {
                    fixture: "effectSleepZeroToYieldNow.ts",
                    matches: main_matches,
                    fixes: main_matches,
                },
                FixCompileCase {
                    fixture: "effectSleepZeroToYieldNow_aliases.ts",
                    matches: 15,
                    fixes: 9,
                },
                FixCompileCase {
                    fixture: "effectSleepZeroToYieldNow_preview.ts",
                    matches: 1,
                    fixes: 1,
                },
            ],
        );
    }
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestEffectSleepZeroToYieldNowFixesCompile".to_string(),
        test_effect_sleep_zero_to_yield_now_fixes_compile,
    )]
}
