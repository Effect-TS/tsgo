//! Rust-only: the baseline outcomes of a run of the `effecttest` test binary.
//!
//! As in Go, a baseline that differs from its reference fails the test with the diff (the actual output is written
//! to `testdata/baselines/local-rs/`). The Go runner owns the shared references, so two more cases fail here:
//!
//! - a missing reference is a mismatch (Go creates it; Rust only writes the local file);
//! - a reference baseline of a case that the Rust runner did not produce fails the case (`report_missing`): the Go
//!   runner wrote it, so the port lost an output.
//!
//! At the end of the run the binary prints one summary line of the outcomes (`finish`).

use std::collections::BTreeMap;
use std::sync::LazyLock;

use ets_go::testing::T;
use parking_lot::Mutex;

use crate::*;

/// What a runner made of one reference baseline.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// Produced and equal to the reference.
    Match,
    /// Produced and different from the reference (or there is no reference).
    Mismatch,
    /// The reference exists but the Rust runner did not produce it.
    Missing,
}

/// Every outcome reported in this run, by baseline path (relative to `testdata/baselines/reference`).
static RESULTS: LazyLock<Mutex<BTreeMap<String, Outcome>>> = LazyLock::new(Mutex::default);

/// Records `outcome` for `baseline` (relative to the reference directory) and fails `t` unless it matches.
/// `failure` renders the message of a mismatch (the diff).
pub fn report(t: &T, baseline: &str, outcome: Outcome, failure: impl FnOnce() -> String) {
    RESULTS.lock().insert(baseline.to_string(), outcome);
    match outcome {
        Outcome::Match => {}
        Outcome::Mismatch => t.error(format!("{baseline}: {}", failure())),
        Outcome::Missing => t.error(format!(
            "{baseline}: the reference baseline exists but the Rust runner does not produce it"
        )),
    }
}

/// The reference baselines below `subfolder` (file names, sorted), read once.
pub fn reference_files(subfolder: &str) -> Vec<String> {
    static CACHE: LazyLock<Mutex<BTreeMap<String, Vec<String>>>> = LazyLock::new(Mutex::default);
    CACHE
        .lock()
        .entry(subfolder.to_string())
        .or_insert_with(|| {
            read_dir_sorted(&baseline_reference_path(subfolder))
                .map(|entries| {
                    entries
                        .into_iter()
                        .filter(|(_, is_dir)| !is_dir)
                        .map(|(name, _)| name)
                        .collect()
                })
                .unwrap_or_default()
        })
        .clone()
}

/// The reference baselines `RunEffectTest` writes for the case `baseline_name`: `.errors.txt`, `.pipings.txt`
/// and `.layers.txt`.
pub fn diagnostics_runner_outputs(subfolder: &str, baseline_name: &str) -> Vec<String> {
    let prefix = format!("{baseline_name}.");
    reference_files(subfolder)
        .into_iter()
        .filter(|name| {
            name.strip_prefix(&prefix)
                .is_some_and(|suffix| matches!(suffix, "errors.txt" | "pipings.txt" | "layers.txt"))
        })
        .map(|name| format!("{subfolder}/{name}"))
        .collect()
}

/// The reference baselines a language-service runner writes for the case `baseline_name` with this suffix
/// (`quickfixes.txt`, `refactors.txt`, `symbols.txt`): the one file, when its reference exists.
pub fn ls_runner_outputs(subfolder: &str, baseline_name: &str, suffix: &str) -> Vec<String> {
    let name = format!("{baseline_name}.{suffix}");
    if reference_files(subfolder).contains(&name) {
        return vec![format!("{subfolder}/{name}")];
    }
    Vec::new()
}

/// Reports every reference baseline in `expected` that this run did not produce for the case as `Missing`
/// (a runner that stops early writes nothing more).
pub fn report_missing(t: &T, expected: &[String]) {
    for baseline in expected {
        let produced = RESULTS.lock().contains_key(baseline);
        if !produced {
            report(t, baseline, Outcome::Missing, String::new);
        }
    }
}

/// Run at the end of the test binary: prints the summary line of the baselines.
pub fn finish() {
    let results = RESULTS.lock();
    let mut matched = 0;
    let mut mismatched = 0;
    let mut missing = 0;
    for outcome in results.values() {
        match outcome {
            Outcome::Match => matched += 1,
            Outcome::Mismatch => mismatched += 1,
            Outcome::Missing => missing += 1,
        }
    }
    println!(
        "\nbaselines: {} checked, {matched} match, {mismatched} mismatched (actual output in \
         testdata/baselines/local-rs), {missing} not produced",
        results.len()
    );
}
