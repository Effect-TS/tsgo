// Package refactors contains all refactor implementations and the registry.
// This mirrors the fixables package structure for code fixes.

use std::sync::LazyLock;

use ets_refactor::Refactor;

use crate::*;

// go: internal/refactors/refactors.go:9 All aa1d03a7ebf7278e
// All is the list of all refactor providers.
// Add new refactors here explicitly - no init() magic.
//
// Rust: Go's whole list, in Go's order; the refactors not ported yet are the commented-out entries
// (porting/PORTING.md, "Porting a fixable, refactor or completion"). Uncomment an entry in place when its refactor
// lands.
pub static ALL: LazyLock<Vec<Refactor>> = LazyLock::new(|| {
    vec![
        (*WRAP_WITH_PIPE).clone(),
        (*TOGGLE_PIPE_STYLE).clone(),
        (*PIPEABLE_TO_DATAFIRST).clone(),
        (*WRAP_WITH_EFFECT_GEN).clone(),
        (*REMOVE_UNNECESSARY_EFFECT_GEN).clone(),
        (*ASYNC_AWAIT_TO_GEN).clone(),
        (*ASYNC_AWAIT_TO_GEN_TRY_PROMISE).clone(),
        (*ASYNC_AWAIT_TO_FN).clone(),
        (*ASYNC_AWAIT_TO_FN_TRY_PROMISE).clone(),
        (*EFFECT_GEN_TO_FN).clone(),
        (*TOGGLE_TYPE_ANNOTATION).clone(),
        (*TOGGLE_RETURN_TYPE_ANNOTATION).clone(),
        (*TOGGLE_LAZY_CONST).clone(),
        (*FUNCTION_TO_ARROW).clone(),
        (*LAYER_MAGIC).clone(),
        (*WRITE_TAG_CLASS_ACCESSORS).clone(),
        (*TYPE_TO_EFFECT_SCHEMA).clone(),
        (*TYPE_TO_EFFECT_SCHEMA_CLASS).clone(),
        (*MAKE_SCHEMA_OPAQUE).clone(),
        (*MAKE_SCHEMA_OPAQUE_WITH_NS).clone(),
        (*STRUCTURAL_TYPE_TO_SCHEMA).clone(),
    ]
});
