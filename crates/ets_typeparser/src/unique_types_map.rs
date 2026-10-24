use ets_readonly::Slice;
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::core::P;
use rustc_hash::FxHashMap;

use crate::*;

// go: internal/typeparser/unique_types_map.go:11 UniqueTypesResult
// UniqueTypesResult holds the result of AppendToUniqueTypesMap.
pub struct UniqueTypesResult {
    pub(crate) all_indexes: Option<Vec<String>>, // All type indexes encountered (both new and existing)
}

impl UniqueTypesResult {
    // go: internal/typeparser/unique_types_map.go:15 UniqueTypesResult.AllIndexes
    pub fn all_indexes(&self) -> Slice<'_, String> {
        ets_readonly::new(self.all_indexes.as_deref())
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/unique_types_map.go:22 TypeParser.AppendToUniqueTypesMap
    // AppendToUniqueTypesMap deduplicates types using bidirectional assignability checks.
    // It unrolls union types, skips excluded types (via shouldExclude), and for each
    // remaining type checks if it's already in the memory map. New types get fresh IDs
    // ("t1", "t2", etc.); known types get their existing IDs.
    // Returns all indexes encountered (both new and known).
    //
    // Rust: Go ranges over `memory` in map order and takes the first equivalent type. The IDs are visited in sorted
    // order here; the types in the map are pairwise inequivalent when built by this function, so at most one
    // matches and the order is not observable. Go's `shouldExclude` closure may capture the parser, so it gets the
    // parser as an argument (PORTING.md, "A Go closure that captures the `*TypeParser`").
    pub fn append_to_unique_types_map(
        &mut self,
        memory: &mut FxHashMap<String, P<Type>>,
        initial_type: Option<P<Type>>,
        mut should_exclude: Option<&mut dyn FnMut(&mut TypeParser<'_>, P<Type>) -> bool>,
    ) -> UniqueTypesResult {
        let mut all_indexes: Option<Vec<String>> = None;
        let mut to_test: Vec<Option<P<Type>>> = vec![initial_type];

        // Pop from the end of the slice
        while let Some(t) = to_test.pop() {
            let Some(t) = t else {
                continue;
            };

            if let Some(should_exclude) = should_exclude.as_mut()
                && should_exclude(self, t)
            {
                continue;
            }

            // If it's a union type, expand its members onto the worklist
            if t.flags().intersects(TypeFlags::Union) {
                to_test.extend(t.types().iter().map(|&member| Some(member)));
                continue;
            }

            // Check if an equivalent type already exists in memory
            let mut matched_id = String::new();
            let mut type_ids: Vec<&String> = memory.keys().collect();
            type_ids.sort_unstable();
            for type_id in type_ids {
                let known_type = memory[type_id];
                if self.checker.is_type_assignable_to(known_type, t)
                    && self.checker.is_type_assignable_to(t, known_type)
                {
                    matched_id.clone_from(type_id);
                    break;
                }
            }

            if matched_id.is_empty() {
                // New type: assign a fresh ID
                let new_id = format!("t{}", memory.len() + 1);
                memory.insert(new_id.clone(), t);
                all_indexes.get_or_insert_with(Vec::new).push(new_id);
            } else {
                // Known type: record the existing ID
                all_indexes.get_or_insert_with(Vec::new).push(matched_id);
            }
        }

        UniqueTypesResult { all_indexes }
    }
}
