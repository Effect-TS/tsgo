// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use libtest_mimic::Trial;

use crate::fixes_compile_test::{FixCompileCase, verify_fixes_compile};
use crate::run_case;

// go test: internal/effecttest/manual_metric_tap_to_track_test.go:11 TestManualMetricTapToTrackFixesCompile d46af2b19d949278
fn test_manual_metric_tap_to_track_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    verify_fixes_compile(
        t,
        EffectVersion::V4,
        "manualMetricTapToTrack",
        tsdiag::EFFECT_0_EXPRESSES_RECORDING_A_METRIC_ON_EVERY_1_DIRECTLY_EFFECT_MANUAL_METRIC_TAP_TO_TRACK.code(),
        |title| title.starts_with("Replace with Effect.track"),
        &[
            FixCompileCase { fixture: "manualMetricTapToTrack.ts", matches: 30, fixes: 30 },
            FixCompileCase { fixture: "manualMetricTapToTrack_preview.ts", matches: 1, fixes: 1 },
            FixCompileCase { fixture: "manualMetricTapToTrack_noTarget.ts", matches: 3, fixes: 0 },
            FixCompileCase { fixture: "manualMetricTapToTrack_bindings.ts", matches: 7, fixes: 6 },
            FixCompileCase { fixture: "manualMetricTapToTrack_localAliases.ts", matches: 3, fixes: 2 },
            FixCompileCase { fixture: "manualMetricTapToTrack_fixShapes.ts", matches: 12, fixes: 11 },
        ],
    );
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestManualMetricTapToTrackFixesCompile".to_string(),
        test_manual_metric_tap_to_track_fixes_compile,
    )]
}
