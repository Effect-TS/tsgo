// Package completions contains all completion implementations and the registry.
// This mirrors the fixables/refactors package structure.

use std::sync::LazyLock;

use ets_completion::Completion;

use crate::*;

// go: internal/completions/completions.go:9 All 668beb8e786c0eb5
// All is the list of all completion providers.
// Add new completions here explicitly - no init() magic.
//
// Rust: Go's whole list, in Go's order (docs/PORTING.md, "Porting a fixable, refactor or completion").
pub static ALL: LazyLock<Vec<Completion>> = LazyLock::new(|| {
    vec![
        (*EFFECT_SCHEMA_SELF_IN_CLASSES).clone(),
        (*EFFECT_DATA_CLASSES).clone(),
        (*CONTEXT_SELF_IN_CLASSES).clone(),
        (*GEN_FUNCTION_STAR).clone(),
        (*FN_FUNCTION_STAR).clone(),
        (*EFFECT_DIAGNOSTICS_COMMENT).clone(),
        (*EFFECT_CODEGENS_COMMENT).clone(),
        (*EFFECT_JSDOC_COMMENT).clone(),
        (*DURATION_INPUT).clone(),
        (*EFFECT_SELF_IN_CLASSES).clone(),
        (*EFFECT_SQL_MODEL_SELF_IN_CLASSES).clone(),
        (*RPC_MAKE_CLASSES).clone(),
        (*SCHEMA_BRAND).clone(),
    ]
});
