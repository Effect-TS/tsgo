//! Rust-only: the test binary of the ported internal/effecttest Go tests. Every Go subtest (one per case of
//! `testdata/tests/<suite>`) is a libtest trial named like Go's (`TestEffectDiagnostics/<case>`), so
//! `cargo test -p ets_effecttest -- <case>` runs one case. A baseline that differs from its reference fails its
//! case, as in Go; at the end the binary prints a summary line of the baselines (`ets_effecttest::baseline_results`).

mod autoimport_style_consistency_test;
mod completion_test;
mod document_symbols_runner_test;
mod effect_fn_pipeables_test;
mod effect_sleep_zero_to_yield_now_test;
mod feature_flags_test;
mod flat_map_identity_to_flatten_test;
mod flat_map_to_tap_test;
mod hover_test;
mod inlay_hints_baseline_test;
mod inlay_hints_test;
mod issue_173_test;
mod quickfix_runner_test;
mod refactor_runner_test;
mod runner_test;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use libtest_mimic::{Arguments, Failed, Trial};

/// The case's name: the test file's base name without `.ts` (Go's `strings.TrimSuffix(filepath.Base(tc), ".ts")`).
pub(crate) fn case_name(tc: &Path) -> String {
    let base = tc.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    base.strip_suffix(".ts").map_or(base.clone(), str::to_string)
}

/// A trial running a Go test body on a thread with the checker's stack (`ets_go::testing::CHECKER_STACK_SIZE`),
/// named after the trial (`ets_go::testing::run` names the test after its thread).
pub(crate) fn run_case(
    name: String,
    body: impl FnOnce(&ets_go::testing::T) + Send + 'static,
) -> Trial {
    Trial::test(name.clone(), move || {
        let thread = std::thread::Builder::new()
            .name(name)
            .stack_size(ets_go::testing::CHECKER_STACK_SIZE)
            .spawn(move || catch_unwind(AssertUnwindSafe(|| ets_go::testing::run(body))))
            .map_err(|err| Failed::from(format!("failed to spawn the test thread: {err}")))?;
        let result = thread.join().unwrap_or_else(Err);
        result.map_err(|payload| {
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "test failed (see its output above)".to_string());
            Failed::from(message)
        })
    })
}

/// Rust-only: Go's `diagnostic.Code != nil && diagnostic.Code.Integer != nil && *diagnostic.Code.Integer == code`.
pub(crate) fn has_diagnostic_code(diagnostic: &ets_shim::lsproto::Diagnostic, code: i32) -> bool {
    diagnostic.code.as_ref().and_then(|code| code.integer) == Some(code)
}

/// The trial standing for a Go test that skips or fails before running its subtests.
pub(crate) fn parent_failure(name: &str, message: String, skip: bool) -> Vec<Trial> {
    if skip {
        println!("{name}: SKIP: {message}");
        return vec![Trial::test(name.to_string(), || Ok(())).with_ignored_flag(true)];
    }
    vec![Trial::test(name.to_string(), move || Err(Failed::from(message)))]
}

fn main() {
    let args = Arguments::from_args();

    let mut trials = Vec::new();
    trials.extend(runner_test::test_effect_diagnostics());
    trials.extend(runner_test::test_effect_v3_diagnostics());
    trials.extend(quickfix_runner_test::test_effect_quick_fixes());
    trials.extend(quickfix_runner_test::test_effect_v3_quick_fixes());
    trials.extend(refactor_runner_test::test_effect_refactors());
    trials.extend(refactor_runner_test::test_effect_v3_refactors());
    trials.extend(document_symbols_runner_test::test_effect_document_symbols());
    trials.extend(document_symbols_runner_test::test_effect_v3_document_symbols());
    trials.extend(hover_test::tests());
    trials.extend(completion_test::tests());
    trials.extend(inlay_hints_test::tests());
    trials.extend(autoimport_style_consistency_test::tests());
    trials.extend(feature_flags_test::tests());
    trials.extend(issue_173_test::tests());
    trials.extend(effect_fn_pipeables_test::tests());
    trials.extend(effect_sleep_zero_to_yield_now_test::tests());
    trials.extend(flat_map_to_tap_test::tests());
    trials.extend(flat_map_identity_to_flatten_test::tests());

    let conclusion = libtest_mimic::run(&args, trials);
    if !args.list {
        ets_effecttest::baseline_results::finish();
    }
    conclusion.exit();
}
