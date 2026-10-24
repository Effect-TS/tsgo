// Package etstesthooks provides automatic Effect package mounting for fourslash tests.
//
// This package registers a PrepareTestFSCallback that detects Effect imports in
// fourslash test files and automatically mounts the real Effect node_modules into
// the test VFS via vfstest.MountEffect.
//
// Import this package with a blank import in test files that use fourslash with Effect:
//
//	import _ "github.com/effect-ts/tsgo/etstesthooks"
//
// Rust: Go's blank import runs the package's `init()`; callers call `ets_etstesthooks::init()` instead. The
// fourslash harness is the Rust-only stand-in `ets_fourslash`.

mod init;
#[cfg(test)]
mod init_test;

/// Rust-only: runs the package's Go `init()` once (docs/PORTING.md, "Go runtime equivalents").
pub fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(init::init);
}
