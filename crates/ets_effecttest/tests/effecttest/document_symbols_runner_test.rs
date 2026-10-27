// Rust: Go's blank import of `etslshooks`; `ets_effecttest::run_effect_document_symbols_test` registers it (`init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_effecttest::baseline_results;
use libtest_mimic::Trial;

use crate::{case_name, parent_failure, run_case};

// go test: internal/effecttest/document_symbols_runner_test.go:14 TestEffectDocumentSymbols 94e541e4814e7d0d
// Rust: one libtest trial per Go subtest (`TestEffectDocumentSymbols/<case>`), as `test_effect_diagnostics`.
pub(crate) fn test_effect_document_symbols() -> Vec<Trial> {
    const NAME: &str = "TestEffectDocumentSymbols";
    if let Err(err) = ets_bundledeffect::ensure_package_installed(EffectVersion::V4, "effect") {
        return parent_failure(NAME, format!("Effect not installed: {err}"), true);
    }

    let cases = match ets_effecttest::discover_document_symbol_test_cases(EffectVersion::V4) {
        Ok(cases) => cases,
        Err(err) => {
            return parent_failure(
                NAME,
                format!("Failed to discover document symbol test cases: {err}"),
                false,
            );
        }
    };

    if cases.is_empty() {
        return parent_failure(
            NAME,
            "No Effect document symbol test cases found".to_string(),
            true,
        );
    }

    let mut trials = Vec::new();
    for tc in cases {
        let name = case_name(&tc);

        trials.push(run_case(format!("{NAME}/{name}"), move |t| {
            t.parallel();
            ets_effecttest::run_effect_document_symbols_test(t, EffectVersion::V4, &tc);
            // Rust-only: a reference baseline the runner did not write (it stopped early).
            baseline_results::report_missing(
                t,
                &baseline_results::ls_runner_outputs(
                    EffectVersion::V4.as_str(),
                    &name,
                    "symbols.txt",
                ),
            );
        }));
    }
    trials
}

// go test: internal/effecttest/document_symbols_runner_test.go:40 TestEffectV3DocumentSymbols 63f927ce7aca81d4
pub(crate) fn test_effect_v3_document_symbols() -> Vec<Trial> {
    const NAME: &str = "TestEffectV3DocumentSymbols";
    if let Err(err) = ets_bundledeffect::ensure_package_installed(EffectVersion::V3, "effect") {
        return parent_failure(NAME, format!("Effect V3 not installed: {err}"), true);
    }

    let cases = match ets_effecttest::discover_document_symbol_test_cases(EffectVersion::V3) {
        Ok(cases) => cases,
        Err(err) => {
            return parent_failure(
                NAME,
                format!("Failed to discover V3 document symbol test cases: {err}"),
                false,
            );
        }
    };

    if cases.is_empty() {
        return parent_failure(
            NAME,
            "No Effect V3 document symbol test cases found".to_string(),
            true,
        );
    }

    let mut trials = Vec::new();
    for tc in cases {
        let name = case_name(&tc);

        trials.push(run_case(format!("{NAME}/{name}"), move |t| {
            t.parallel();
            ets_effecttest::run_effect_document_symbols_test(t, EffectVersion::V3, &tc);
            // Rust-only: see test_effect_document_symbols.
            baseline_results::report_missing(
                t,
                &baseline_results::ls_runner_outputs(
                    EffectVersion::V3.as_str(),
                    &name,
                    "symbols.txt",
                ),
            );
        }));
    }
    trials
}
