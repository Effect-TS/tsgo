// Package fixable defines the Fixable type for code fix providers.
// This mirrors the Rule pattern for diagnostics - Fixable defines what a code fix is,
// while individual fixable implementations live in internal/fixables.

use ets_shim::ls::CodeAction;

use crate::*;

// go: internal/fixable/fixable.go:14 Fixable 7789469028be9f87
// Fixable defines a code fix provider for a set of diagnostic codes.
// This is analogous to how Rule defines a diagnostic provider.
//
// Rust: fixables are package-level variables in Go (`LazyLock` statics, as `ets_rule::Rule`s are), so the strings
// are `'static` and `run` is a function pointer. `ErrorCodes` is computed from the diagnostics messages at
// initialization, as in Go (`Message::code` is not `const`).
#[derive(Clone)]
pub struct Fixable {
    // Name is the unique identifier for this fixable (e.g., "effectDisable", "floatingEffectYield").
    pub name: &'static str,

    // Description explains what this fixable does (used in documentation/tooling).
    pub description: &'static str,

    // ErrorCodes is the set of diagnostic codes this fixable handles.
    // A fixable can handle multiple related error codes.
    pub error_codes: Vec<i32>,

    // FixIDs for "fix all" support (optional). Each fix action can have its own ID.
    pub fix_ids: Vec<&'static str>,

    // Run executes the fixable and returns code actions for the given context.
    // The Context provides access to the source file, span, error code,
    // type checker, and tracker lifecycle helpers.
    // Return nil or empty slice if no fixes are applicable.
    pub run: fn(ctx: &mut Context<'_, '_>) -> Vec<CodeAction>,
}

impl Fixable {
    // go: internal/fixable/fixable.go:36 Fixable.HandlesCode 2acde446f2ea044d
    // HandlesCode returns true if this fixable handles the given error code.
    pub fn handles_code(&self, code: i32) -> bool {
        self.error_codes.contains(&code)
    }
}
