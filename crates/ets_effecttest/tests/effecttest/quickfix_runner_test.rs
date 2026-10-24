// Register fourslash VFS callback to mount Effect packages
// Register Effect code fix provider for LSP code actions
// Rust: `ets_effecttest::run_effect_quick_fix_test` registers them (`init_hooks`: `etstesthooks`, `etslshooks`).

use ets_bundledeffect::EffectVersion;
use ets_effecttest::baseline_results;
use libtest_mimic::Trial;

use crate::{case_name, parent_failure, run_case};

// go test: internal/effecttest/quickfix_runner_test.go:17 TestEffectQuickFixes
// Rust: one libtest trial per Go subtest (`TestEffectQuickFixes/<case>`), as `test_effect_diagnostics`.
pub(crate) fn test_effect_quick_fixes() -> Vec<Trial> {
    const NAME: &str = "TestEffectQuickFixes";
    if let Err(err) = ets_bundledeffect::ensure_package_installed(EffectVersion::V4, "effect") {
        return parent_failure(NAME, format!("Effect not installed: {err}"), true);
    }

    let cases = match ets_effecttest::discover_test_cases(EffectVersion::V4) {
        Ok(cases) => cases,
        Err(err) => {
            return parent_failure(NAME, format!("Failed to discover test cases: {err}"), false);
        }
    };

    if cases.is_empty() {
        return parent_failure(NAME, "No Effect test cases found".to_string(), true);
    }

    let mut trials = Vec::new();
    for tc in cases {
        let name = case_name(&tc);

        trials.push(run_case(format!("{NAME}/{name}"), move |t| {
            t.parallel();
            ets_effecttest::run_effect_quick_fix_test(t, EffectVersion::V4, &tc);
            // Rust-only: a reference baseline the runner did not write (it stopped early).
            baseline_results::report_missing(
                t,
                &baseline_results::ls_runner_outputs(
                    EffectVersion::V4.as_str(),
                    &name,
                    "quickfixes.txt",
                ),
            );
        }));
    }
    trials
}

// go test: internal/effecttest/quickfix_runner_test.go:43 TestEffectV3QuickFixes
pub(crate) fn test_effect_v3_quick_fixes() -> Vec<Trial> {
    const NAME: &str = "TestEffectV3QuickFixes";
    if let Err(err) = ets_bundledeffect::ensure_package_installed(EffectVersion::V3, "effect") {
        return parent_failure(NAME, format!("Effect V3 not installed: {err}"), true);
    }

    let cases = match ets_effecttest::discover_test_cases(EffectVersion::V3) {
        Ok(cases) => cases,
        Err(err) => {
            return parent_failure(NAME, format!("Failed to discover V3 test cases: {err}"), false);
        }
    };

    if cases.is_empty() {
        return parent_failure(NAME, "No Effect V3 test cases found".to_string(), true);
    }

    let mut trials = Vec::new();
    for tc in cases {
        let name = case_name(&tc);

        trials.push(run_case(format!("{NAME}/{name}"), move |t| {
            t.parallel();
            ets_effecttest::run_effect_quick_fix_test(t, EffectVersion::V3, &tc);
            // Rust-only: see test_effect_quick_fixes.
            baseline_results::report_missing(
                t,
                &baseline_results::ls_runner_outputs(
                    EffectVersion::V3.as_str(),
                    &name,
                    "quickfixes.txt",
                ),
            );
        }));
    }
    trials
}
