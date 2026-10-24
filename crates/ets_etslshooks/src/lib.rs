// Package etslshooks provides Effect code fix integration with TypeScript-Go.
//
// This package registers a CodeFixProvider that delegates to the fixables
// in internal/fixables. It should be imported (with a blank identifier) by
// the main entry point to register the Effect code fix provider with the
// TypeScript-Go language service.
//
// The package provides:
//   - EffectFixProvider: A CodeFixProvider that handles all Effect diagnostic codes
//   - Automatic delegation to internal/fixables for fix implementations
//
// Usage:
//
//	import _ "github.com/effect-ts/tsgo/etslshooks"
//
// Rust: Go's blank import runs the package's `init()`; callers call `ets_etslshooks::init()` instead. The hook
// points are tsrs patches 011 (code fixes, refactors), 012 (hover), 015 (inlay hints), 022 (completions) and 024
// (document symbols), plus 018 for the auto-import fix transformer.

mod document_symbols;
mod init;
mod inlay_hints;

/// Rust-only: runs the package's Go `init()` once (docs/PORTING.md, "Go runtime equivalents").
pub fn init() {
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(init::init);
}
