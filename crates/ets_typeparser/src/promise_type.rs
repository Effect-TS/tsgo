use ets_shim::checker::{Checker, Type};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/promise_type.go:9 TypeParser.PromiseType 842a9ebfad5184a7
    // PromiseType returns t when it is the global Promise type or a reference to it.
    // It intentionally does not match arbitrary thenables or PromiseLike values.
    //
    // Rust: Go's `getGlobalPromiseTypeChecked` is a resolver field, never nil; tsrs's resolver is a method that
    // always returns a type (`emptyGenericType` when Promise is missing).
    pub fn promise_type(&mut self, t: impl Into<Option<P<Type>>>) -> Option<P<Type>> {
        let t = t.into()?;

        cached(
            self,
            |l| &mut l.promise_type,
            t,
            |tp| {
                let global_promise_type = tp.checker.get_global_promise_type_checked();
                if global_promise_type == tp.checker.empty_generic_type {
                    return None;
                }

                if Checker::is_reference_to_type(Some(t), global_promise_type)
                    || t == global_promise_type
                {
                    return Some(t);
                }

                None
            },
        )
    }
}
