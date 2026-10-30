// Register fourslash VFS callback to mount Effect packages
// Register Effect code fix provider for LSP code actions
// Rust: `ets_effecttest::run_effect_quick_fix_test` registers them (`init_hooks`: `etstesthooks`, `etslshooks`).

use ets_bundledeffect::EffectVersion;
use ets_effecttest::baseline_results;
use libtest_mimic::Trial;

use crate::runner_test::run_fixture_cases;

// go test: internal/effecttest/quickfix_runner_test.go:13 TestEffectQuickFixes e2de2ac8bc488806
pub(crate) fn test_effect_quick_fixes() -> Vec<Trial> {
    run_fixture_cases(
        "TestEffectQuickFixes",
        EffectVersion::V4,
        ets_effecttest::discover_test_cases,
        ets_effecttest::run_effect_quick_fix_test,
        |subfolder, name| baseline_results::ls_runner_outputs(subfolder, name, "quickfixes.txt"),
    )
}

// go test: internal/effecttest/quickfix_runner_test.go:18 TestEffectV3QuickFixes b277cb24dc1f9ff0
pub(crate) fn test_effect_v3_quick_fixes() -> Vec<Trial> {
    run_fixture_cases(
        "TestEffectV3QuickFixes",
        EffectVersion::V3,
        ets_effecttest::discover_test_cases,
        ets_effecttest::run_effect_quick_fix_test,
        |subfolder, name| baseline_results::ls_runner_outputs(subfolder, name, "quickfixes.txt"),
    )
}
