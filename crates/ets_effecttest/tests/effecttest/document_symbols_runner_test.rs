// Rust: Go's blank import of `etslshooks`; `ets_effecttest::run_effect_document_symbols_test` registers it (`init_hooks`).

use ets_bundledeffect::EffectVersion;
use ets_effecttest::baseline_results;
use libtest_mimic::Trial;

use crate::runner_test::run_fixture_cases;

// go test: internal/effecttest/document_symbols_runner_test.go:12 TestEffectDocumentSymbols 8ed6d001d8debd96
pub(crate) fn test_effect_document_symbols() -> Vec<Trial> {
    run_fixture_cases(
        "TestEffectDocumentSymbols",
        EffectVersion::V4,
        ets_effecttest::discover_document_symbol_test_cases,
        ets_effecttest::run_effect_document_symbols_test,
        |subfolder, name| baseline_results::ls_runner_outputs(subfolder, name, "symbols.txt"),
    )
}

// go test: internal/effecttest/document_symbols_runner_test.go:17 TestEffectV3DocumentSymbols 61550dc4b39106cb
pub(crate) fn test_effect_v3_document_symbols() -> Vec<Trial> {
    run_fixture_cases(
        "TestEffectV3DocumentSymbols",
        EffectVersion::V3,
        ets_effecttest::discover_document_symbol_test_cases,
        ets_effecttest::run_effect_document_symbols_test,
        |subfolder, name| baseline_results::ls_runner_outputs(subfolder, name, "symbols.txt"),
    )
}
