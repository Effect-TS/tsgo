// Package etsoxlintrunner exposes Effect diagnostics and suggestions to Oxlint.
//
// Rust: Go's `init()` runs on import; callers call `ets_etsoxlintrunner::init()` instead.

mod runner;
pub use runner::*;

#[cfg(test)]
mod runner_test;

/// Rust-only: runs the package's Go `init()` once (docs/PORTING.md, "Go runtime equivalents").
pub fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(runner::init);
}
