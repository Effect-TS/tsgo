// Package etsexecutehooks provides command-line filtering for Effect diagnostics.
//
// This package registers a FilterDiagnosticsForExitCodeCallback that filters out
// Effect diagnostics from exit-code and noEmitOnError determination based on the
// IgnoreEffectSuggestionsInTscExitCode and IgnoreEffectWarningsInTscExitCode
// configuration options.
//
// Rust: Go's blank import runs the package's `init()`; the binary calls `ets_etsexecutehooks::init()` instead.

mod init;
pub use init::*;

#[cfg(test)]
mod init_test;

/// Rust-only: runs the package's Go `init()` once (docs/PORTING.md, "Go runtime equivalents").
pub fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(init::init);
}
