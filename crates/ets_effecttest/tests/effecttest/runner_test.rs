use ets_bundledeffect::EffectVersion;
use ets_effecttest::baseline_results;
use libtest_mimic::Trial;

use crate::{case_name, parent_failure, run_case};

// go test: internal/effecttest/runner_test.go:11 TestEffectDiagnostics 848c4cd03e5d901e
// Rust: Go's `t.Run(name, ...)` subtests are one libtest trial each, named `TestEffectDiagnostics/<case>`; a
// failure of the parent before its subtests run (no Effect, no cases) is one failing (or ignored) trial.
pub(crate) fn test_effect_diagnostics() -> Vec<Trial> {
    const NAME: &str = "TestEffectDiagnostics";
    // Skip if Effect not installed
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
            ets_effecttest::run_effect_test(t, EffectVersion::V4, &tc);
            // Rust-only: the case's reference baselines the runner did not write (the parts not ported yet).
            baseline_results::report_missing(
                t,
                &baseline_results::diagnostics_runner_outputs(EffectVersion::V4.as_str(), &name),
            );
        }));
    }
    trials
}

// go test: internal/effecttest/runner_test.go:38 TestEffectV3Diagnostics 6e2dcc9ef53541f1
pub(crate) fn test_effect_v3_diagnostics() -> Vec<Trial> {
    const NAME: &str = "TestEffectV3Diagnostics";
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
            ets_effecttest::run_effect_test(t, EffectVersion::V3, &tc);
            // Rust-only: see test_effect_diagnostics.
            baseline_results::report_missing(
                t,
                &baseline_results::diagnostics_runner_outputs(EffectVersion::V3.as_str(), &name),
            );
        }));
    }
    trials
}
