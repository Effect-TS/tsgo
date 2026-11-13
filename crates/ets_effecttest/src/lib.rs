//! Port of internal/effecttest: the baseline runner. It reads the cases in `testdata/tests/<suite>` in place,
//! writes the baselines the Go runner writes to `testdata/baselines/local-rs/` (git-ignored) and compares them with
//! the shared `testdata/baselines/reference/`; a mismatch fails the test, as in Go (`baseline_results`). The Go tests of the package are the `effecttest` test binary (`tests/effecttest`).

mod baseline;
pub mod baseline_results;
#[cfg(test)]
mod conditional_type_relationships_test;
mod document_symbols_baseline;
mod document_symbols_runner;
#[cfg(test)]
mod effect_in_failure_ts2589_test;
mod harness;
mod quickfix_baseline;
mod quickfix_runner;
mod refactor_baseline;
mod refactor_runner;
mod runner;
mod sharedfs;
mod vfs;

pub use baseline::*;
pub use document_symbols_baseline::*;
pub use document_symbols_runner::*;
pub use harness::*;
pub use quickfix_baseline::*;
pub use quickfix_runner::*;
pub use refactor_baseline::*;
pub use refactor_runner::*;
pub use runner::*;
pub use vfs::*;
