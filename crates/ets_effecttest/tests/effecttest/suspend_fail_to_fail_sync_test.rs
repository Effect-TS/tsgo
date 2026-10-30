// Rust: Go's test binary links the hooks through the package's other test files' blank imports; each test
// registers them (`ets_effecttest::init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_go::testing::T;
use ets_shim::diagnostics as tsdiag;
use libtest_mimic::Trial;

use crate::fixes_compile_test::{FixCompileCase, verify_fixes_compile};
use crate::run_case;

// go test: internal/effecttest/suspend_fail_to_fail_sync_test.go:11 TestSuspendFailToFailSyncFixesCompile d4749d8504386972
fn test_suspend_fail_to_fail_sync_fixes_compile(t: &T) {
    ets_effecttest::init_hooks();
    t.parallel();
    for version in [EffectVersion::V3, EffectVersion::V4] {
        t.run(version.as_str(), |t| {
            t.parallel();
            let mut cases = vec![
                FixCompileCase { fixture: "suspendFailToFailSync.ts", matches: 25, fixes: 24 },
                FixCompileCase { fixture: "suspendFailToFailSync_shapes.ts", matches: 18, fixes: 17 },
                FixCompileCase { fixture: "suspendFailToFailSync_exactTypes.ts", matches: 6, fixes: 5 },
            ];
            if version == EffectVersion::V4 {
                cases = vec![
                    FixCompileCase { fixture: "suspendFailToFailSync.ts", matches: 20, fixes: 19 },
                    FixCompileCase { fixture: "suspendFailToFailSync_preview.ts", matches: 1, fixes: 1 },
                    FixCompileCase { fixture: "suspendFailToFailSync_shapes.ts", matches: 15, fixes: 14 },
                    FixCompileCase { fixture: "suspendFailToFailSync_exactTypes.ts", matches: 6, fixes: 5 },
                ];
            }
            verify_fixes_compile(
                t,
                version,
                "suspendFailToFailSync",
                tsdiag::EFFECT_0_BUILDS_THIS_1_LAZILY_DIRECTLY_INSTEAD_OF_WRAPPING_EFFECT_2_IN_EFFECT_SUSPEND_EFFECT_SUSPEND_FAIL_TO_FAIL_SYNC.code(),
                |title| title.starts_with("Replace with Effect."),
                &cases,
            );
        });
    }
}

pub(crate) fn tests() -> Vec<Trial> {
    vec![run_case(
        "TestSuspendFailToFailSyncFixesCompile".to_string(),
        test_suspend_fail_to_fail_sync_fixes_compile,
    )]
}
