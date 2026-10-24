// Package etscheckerhooks provides the initialization and registration of Effect diagnostics
// hooks for the TypeScript-Go checker.
//
// This package should be imported (with a blank identifier) by the main entry point
// to register the Effect diagnostics callback with the TypeScript-Go checker.
//
// Rust: Go's blank import runs the package's `init()`; callers call `ets_etscheckerhooks::init()` instead.

mod init;

/// Rust-only: runs the package's Go `init()` once (docs/PORTING.md, "Go runtime equivalents").
pub fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(init::init);
}
