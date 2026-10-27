// Package fixables contains all code fix implementations and the registry.
// This mirrors the rules package structure for diagnostics.

use std::sync::LazyLock;

use ets_fixable::Fixable;
use rustc_hash::FxHashSet;

use crate::*;

// go: internal/fixables/fixables.go:9 All
// All is the list of all code fix providers.
// Add new fixables here explicitly - no init() magic.
//
// Rust: Go's whole list, in Go's order; the fixables not ported yet are the commented-out entries (docs/PORTING.md,
// "Porting a fixable, refactor or completion"). Uncomment an entry in place when its fixable lands.
pub static ALL: LazyLock<Vec<Fixable>> = LazyLock::new(|| {
    vec![
        // Disable actions (handles all Effect diagnostic codes)
        (*EFFECT_DISABLE).clone(),
        // Add future fixables here:
        // POCRuleFix,
        (*FLOATING_EFFECT_YIELD_FIX).clone(),
        (*MISSING_RETURN_YIELD_STAR_FIX).clone(),
        (*MISSING_STAR_IN_YIELD_EFFECT_GEN_FIX).clone(),
        (*CATCH_DIE_TO_OR_DIE_FIX).clone(),
        (*TIMEOUT_CATCH_TAG_TO_TIMEOUT_OR_ELSE_FIX).clone(),
        (*CATCH_ALL_TO_MAP_ERROR_FIX).clone(),
        (*CATCH_ALL_TAG_DISPATCH_TO_CATCH_TAG_FIX).clone(),
        (*CATCH_IF_TAG_TO_CATCH_TAG_FIX).clone(),
        (*CATCH_TAG_TO_CATCH_REASON_FIX).clone(),
        (*ALL_OF_MAP_TO_FOR_EACH_FIX).clone(),
        (*FLAT_MAP_TO_MAP_FIX).clone(),
        (*FLAT_MAP_TO_TAP_FIX).clone(),
        (*FLAT_MAP_IGNORED_PARAM_TO_AND_THEN_FIX).clone(),
        (*FLAT_MAP_IDENTITY_TO_FLATTEN_FIX).clone(),
        (*MATCH_EFFECT_TO_MATCH_FIX).clone(),
        (*MATCH_EFFECT_TO_MAP_BOTH_FIX).clone(),
        (*MAP_MAP_ERROR_TO_MAP_BOTH_FIX).clone(),
        (*FLAT_MAP_CONDITIONAL_TO_FILTER_OR_FAIL_FIX).clone(),
        (*MAP_SOME_TO_AS_SOME_FIX).clone(),
        (*MAP_CONST_TO_AS_FIX).clone(),
        (*MANUAL_METRIC_TAP_TO_TRACK_FIX).clone(),
        (*FAIL_CAUSE_TO_CONSTRUCTOR_FIX).clone(),
        (*OPTION_MATCH_TO_FROM_OPTION_FIX).clone(),
        (*SYNC_TO_SUCCEED_FIX).clone(),
        (*EFFECT_SLEEP_ZERO_TO_YIELD_NOW_FIX).clone(),
        (*CATCH_TO_OR_ELSE_SUCCEED_FIX).clone(),
        (*CATCH_TO_IGNORE_FIX).clone(),
        (*EFFECT_FN_IIFE_FIX).clone(),
        (*UNNECESSARY_PIPE_FIX).clone(),
        (*UNNECESSARY_PIPE_CHAIN_FIX).clone(),
        (*RETURN_EFFECT_IN_GEN_FIX).clone(),
        (*EFFECT_SUCCEED_WITH_VOID_FIX).clone(),
        (*PREFER_SUCCEED_SOME_OR_NONE_FIX).clone(),
        (*UNNECESSARY_EFFECT_GEN_FIX).clone(),
        (*UNNECESSARY_ARROW_BLOCK_FIX).clone(),
        (*UNNECESSARY_TYPEOF_TYPE_FIX).clone(),
        (*PREFER_SCHEMA_TYPE_PROPERTY_FIX).clone(),
        (*EFFECT_MAP_VOID_FIX).clone(),
        (*UNNECESSARY_FAIL_YIELDABLE_ERROR_FIX).clone(),
        (*CLASS_SELF_MISMATCH_FIX).clone(),
        (*EFFECT_FN_OPPORTUNITY_FIX).clone(),
        (*DETERMINISTIC_KEYS_FIX).clone(),
        (*SCOPE_IN_LAYER_EFFECT_SCOPED_FIX).clone(),
        (*OVERRIDDEN_SCHEMA_CONSTRUCTOR_FIX).clone(),
        (*INSTANCE_OF_SCHEMA_FIX).clone(),
        (*LAYER_MERGE_ALL_WITH_DEPENDENCIES_FIX).clone(),
        (*UNSAFE_EFFECT_TYPE_ASSERTION_FIX).clone(),
        (*MISSING_EFFECT_ERROR_CATCH_FIX).clone(),
        (*MULTIPLE_EFFECT_PROVIDE_FIX).clone(),
        (*PROVIDE_LAYER_SUCCEED_TO_PROVIDE_SERVICE_FIX).clone(),
        (*SCHEMA_STRUCT_WITH_TAG_FIX).clone(),
        (*SCHEMA_NUMBER_FIX).clone(),
        (*REDUNDANT_SCHEMA_TAG_IDENTIFIER_REMOVE_IDENTIFIER_FIX).clone(),
        (*RUN_EFFECT_INSIDE_EFFECT_FIX).clone(),
        (*SCHEMA_UNION_OF_LITERALS_FIX).clone(),
        (*MISSED_PIPEABLE_OPPORTUNITY_FIX).clone(),
        (*NEW_SCHEMA_CLASS_FIX).clone(),
        (*SERVICE_NOT_AS_CLASS_FIX).clone(),
        (*PREFER_UNSAFE_CONSTRUCTOR_FIX).clone(),
        (*PREFER_TYPED_SCHEMA_DECODER_FIX).clone(),
        (*ACQUIRE_RELEASE_DISPOSABLE_FIX).clone(),
        (*RUN_OF_EXIT_TO_RUN_EXIT_FIX).clone(),
    ]
});

// go: internal/fixables/fixables.go:79 ByErrorCode
// ByErrorCode finds all fixables that handle a given error code.
pub fn by_error_code(code: i32) -> Vec<&'static Fixable> {
    let mut results = Vec::new();
    for fixable in ALL.iter() {
        if fixable.handles_code(code) {
            results.push(fixable);
        }
    }
    results
}

// go: internal/fixables/fixables.go:90 AllErrorCodes
// AllErrorCodes returns all unique error codes handled by any fixable.
pub fn all_error_codes() -> Vec<i32> {
    let mut seen: FxHashSet<i32> = FxHashSet::default();
    let mut codes = Vec::new();
    for f in ALL.iter() {
        for &code in &f.error_codes {
            if seen.insert(code) {
                codes.push(code);
            }
        }
    }
    codes
}

// go: internal/fixables/fixables.go:105 AllFixIDs
// AllFixIDs returns all unique fix IDs from all fixables.
pub fn all_fix_ids() -> Vec<&'static str> {
    let mut seen: FxHashSet<&'static str> = FxHashSet::default();
    let mut ids = Vec::new();
    for f in ALL.iter() {
        for &id in &f.fix_ids {
            if seen.insert(id) {
                ids.push(id);
            }
        }
    }
    ids
}
