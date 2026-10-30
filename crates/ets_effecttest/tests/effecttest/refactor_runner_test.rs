// Register fourslash VFS callback to mount Effect packages
// Register Effect code fix and refactor providers for LSP code actions
// Rust: `ets_effecttest::run_effect_refactor_test` registers them (`init_hooks`: `etstesthooks`, `etslshooks`).

use ets_bundledeffect::EffectVersion;
use ets_effecttest::baseline_results;
use libtest_mimic::Trial;

use crate::runner_test::run_fixture_cases;

// go test: internal/effecttest/refactor_runner_test.go:13 TestEffectRefactors f6fb48c10d96b23f
pub(crate) fn test_effect_refactors() -> Vec<Trial> {
    run_fixture_cases(
        "TestEffectRefactors",
        EffectVersion::V4,
        ets_effecttest::discover_refactor_test_cases,
        ets_effecttest::run_effect_refactor_test,
        |subfolder, name| baseline_results::ls_runner_outputs(subfolder, name, "refactors.txt"),
    )
}

// go test: internal/effecttest/refactor_runner_test.go:18 TestEffectV3Refactors 1dc9ff72b2a08948
pub(crate) fn test_effect_v3_refactors() -> Vec<Trial> {
    run_fixture_cases(
        "TestEffectV3Refactors",
        EffectVersion::V3,
        ets_effecttest::discover_refactor_test_cases,
        ets_effecttest::run_effect_refactor_test,
        |subfolder, name| baseline_results::ls_runner_outputs(subfolder, name, "refactors.txt"),
    )
}
