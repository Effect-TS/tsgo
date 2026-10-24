// Package completion defines the Completion type for custom completion providers.
// This mirrors the rule/fixable/refactor pattern — Completion defines what a custom
// completion is, while individual implementations live in internal/completions.

use ets_shim::lsproto;

use crate::*;

// go: internal/completion/completion.go:13 Completion
// Completion defines a custom completion provider.
// Each completion has a name, description, and a Run function that returns
// additional completion items to merge into the existing completion list.
//
// Rust: completions are package-level variables in Go (`LazyLock` statics, as `ets_fixable::Fixable`s are), so the
// strings are `'static` and `run` is a function pointer.
#[derive(Clone)]
pub struct Completion {
    // Name is the unique identifier for this completion (e.g., "genFunctionStar").
    pub name: &'static str,

    // Description explains what this completion provides (used in documentation/tooling).
    pub description: &'static str,

    // Run executes the completion and returns additional completion items.
    // The Context provides access to the source file, cursor position,
    // existing completion items, and type checker.
    // Return nil or empty slice if no custom completions are applicable.
    pub run: fn(ctx: &mut Context<'_, '_>) -> Vec<lsproto::CompletionItem>,
}
