// Package refactor defines the Refactor type for selection-based refactoring providers.
// This mirrors the fixable.Fixable pattern — Refactor defines what a refactor is,
// while individual refactor implementations live in internal/refactors.

use ets_shim::ls::CodeAction;

use crate::*;

// go: internal/refactor/refactor.go:13 Refactor 4970b85bef66b3a2
// Refactor defines a selection-based refactoring action.
// Unlike Fixable, refactors are not tied to a specific diagnostic error code —
// they apply based on the user's selection.
//
// Rust: refactors are package-level variables in Go (`LazyLock` statics, as `ets_fixable::Fixable`s are), so the
// strings are `'static` and `run` is a function pointer.
#[derive(Clone)]
pub struct Refactor {
    // Name is the unique identifier for this refactor (e.g., "wrapWithPipe").
    pub name: &'static str,

    // Description explains what this refactor does (shown in the editor's refactor menu).
    pub description: &'static str,

    // Kind is the CodeActionKind suffix (e.g., "rewrite.effect.wrapWithPipe").
    pub kind: &'static str,

    // Run executes the refactor and returns code actions for the given context.
    // The Context provides access to the source file, span, type checker,
    // and tracker lifecycle helpers.
    // Return nil or empty slice if the refactor is not applicable.
    pub run: fn(ctx: &mut Context<'_, '_>) -> Vec<CodeAction>,
}
