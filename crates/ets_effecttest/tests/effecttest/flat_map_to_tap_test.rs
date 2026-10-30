// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use libtest_mimic::Trial;

use crate::fixes_compile_test::{FixCompileCase, verify_fixes_compile};
use crate::run_case;

// go test: internal/effecttest/flat_map_to_tap_test.go:11 TestFlatMapToTapFixesCompile 0ae1c249726a52a8
fn test_flat_map_to_tap_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    let code = tsdiag::EFFECT_TAP_EXPRESSES_THIS_OBSERVATION_MORE_DIRECTLY_THAN_EFFECT_0_WITH_A_MANUAL_RE_YIELD_EFFECT_FLAT_MAP_TO_TAP.code();
    for (version, main_matches) in [(EffectVersion::V3, 22), (EffectVersion::V4, 23)] {
        verify_fixes_compile(
            t,
            version,
            "flatMapToTap",
            code,
            |title| title.starts_with("Replace with Effect.tap"),
            &[
                FixCompileCase {
                    fixture: "flatMapToTap.ts",
                    matches: main_matches,
                    fixes: main_matches,
                },
                FixCompileCase { fixture: "flatMapToTap_aliases.ts", matches: 6, fixes: 6 },
                FixCompileCase { fixture: "flatMapToTap_preview.ts", matches: 1, fixes: 1 },
                FixCompileCase { fixture: "flatMapToTap_noTarget.ts", matches: 1, fixes: 0 },
                FixCompileCase { fixture: "flatMapToTap_comments.ts", matches: 2, fixes: 2 },
            ],
        );
    }
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case("TestFlatMapToTapFixesCompile".to_string(), test_flat_map_to_tap_fixes_compile)]
}
