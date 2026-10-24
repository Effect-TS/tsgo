use ets_shim::ast::SymbolFlags;
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/global_error.go:12 TypeParser.IsGlobalErrorType
    // IsGlobalErrorType reports whether the given type is exactly the global Error type.
    // It performs a bidirectional assignability check to ensure the type is not a subclass
    // or unrelated type. Types like any and unknown are excluded since they are
    // bidirectionally assignable to everything and would produce false positives.
    pub fn is_global_error_type(&mut self, t: impl Into<Option<P<Type>>>) -> bool {
        let Some(t) = t.into() else {
            return false;
        };
        cached(
            self,
            |l| &mut l.is_global_error_type,
            t,
            |tp| {
                // Exclude any/unknown — they are bidirectionally assignable to everything
                if t.flags().intersects(TypeFlags::Any | TypeFlags::Unknown) {
                    return false;
                }

                let Some(error_symbol) =
                    tp.checker.resolve_name_exported("Error", None, SymbolFlags::Type, false)
                else {
                    return false;
                };

                let global_error_type =
                    tp.checker.get_declared_type_of_symbol_exported(error_symbol);

                tp.checker.is_type_assignable_to(t, global_error_type)
                    && tp.checker.is_type_assignable_to(global_error_type, t)
            },
        )
    }
}
