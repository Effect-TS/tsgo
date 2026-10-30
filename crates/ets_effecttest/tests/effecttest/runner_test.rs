use std::path::{Path, PathBuf};

use ets_bundledeffect::EffectVersion;
use ets_effecttest::baseline_results;
use ets_go::testing::T;
use libtest_mimic::Trial;

use crate::{case_name, parent_failure, run_case};

// go test: internal/effecttest/runner_test.go:12 runFixtureCases 549e9d3f195f38a1
// Rust: Go's `t.Run(name, ...)` subtests are one libtest trial each, named `<name>/<case>`; a failure of the parent
// before its subtests run (no Effect, no cases) is one failing (or ignored) trial. `outputs` lists the case's
// reference baselines the runner must write (`baseline_results::report_missing`).
pub(crate) fn run_fixture_cases(
    name: &'static str,
    version: EffectVersion,
    discover: fn(EffectVersion) -> std::io::Result<Vec<PathBuf>>,
    run: fn(&T, EffectVersion, &Path),
    outputs: fn(&str, &str) -> Vec<String>,
) -> Vec<Trial> {
    if let Err(err) = ets_bundledeffect::ensure_package_installed(version, "effect") {
        return parent_failure(name, format!("Effect not installed: {err}"), true);
    }

    let cases = match discover(version) {
        Ok(cases) => cases,
        Err(err) => {
            return parent_failure(name, format!("Failed to discover test cases: {err}"), false);
        }
    };

    if cases.is_empty() {
        return parent_failure(name, "No test cases found".to_string(), true);
    }

    let mut trials = Vec::new();
    for tc in cases {
        let case = case_name(&tc);
        trials.push(run_case(format!("{name}/{case}"), move |t| {
            t.parallel();
            run(t, version, &tc);
            // Rust-only: the case's reference baselines the runner did not write (it stopped early).
            baseline_results::report_missing(t, &outputs(version.as_str(), &case));
        }));
    }
    trials
}

// go test: internal/effecttest/runner_test.go:40 TestEffectDiagnostics 6244e84f09f801ee
pub(crate) fn test_effect_diagnostics() -> Vec<Trial> {
    run_fixture_cases(
        "TestEffectDiagnostics",
        EffectVersion::V4,
        ets_effecttest::discover_test_cases,
        ets_effecttest::run_effect_test,
        baseline_results::diagnostics_runner_outputs,
    )
}

// go test: internal/effecttest/runner_test.go:45 TestEffectV3Diagnostics 7a22cab55ee62cac
pub(crate) fn test_effect_v3_diagnostics() -> Vec<Trial> {
    run_fixture_cases(
        "TestEffectV3Diagnostics",
        EffectVersion::V3,
        ets_effecttest::discover_test_cases,
        ets_effecttest::run_effect_test,
        baseline_results::diagnostics_runner_outputs,
    )
}
