use ets_shim::ast::SourceFile;
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::compiler::Program;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/yieldable_error.go:11 sourceFileProgram
// sourceFileProgram is a local interface for accessing program source files.
pub(crate) trait SourceFileProgram {
    fn source_files(&self) -> &'static [P<SourceFile>];
}

// go: internal/typeparser/yieldable_error.go:11 sourceFileProgram
impl SourceFileProgram for Program {
    fn source_files(&self) -> &'static [P<SourceFile>] {
        Program::source_files(self)
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/yieldable_error.go:17 TypeParser.IsYieldableErrorType
    // IsYieldableErrorType reports whether the given type is assignable to Cause.YieldableError
    // from the "effect" package. Returns false for never and any types.
    //
    // Rust: Go's assertion to `sourceFileProgram` succeeds for the compiler program (see `TypeParser`).
    pub fn is_yieldable_error_type(&mut self, t: impl Into<Option<P<Type>>>) -> bool {
        let Some(t) = t.into() else {
            return false;
        };
        cached(
            self,
            |l| &mut l.is_yieldable_error_type,
            t,
            |tp| {
                // never is assignable to everything, so we need to exclude it
                if t.flags().intersects(TypeFlags::Never) {
                    return false;
                }
                // any is assignable to everything, so we need to exclude it
                if t.flags().intersects(TypeFlags::Any) {
                    return false;
                }

                let prog: &dyn SourceFileProgram = tp.program;

                for &sf in prog.source_files() {
                    // Check this source file belongs to the "effect" package
                    let Some(pkg) = tp.package_json_for_source_file(sf) else {
                        continue;
                    };
                    let Some(name) = pkg.name.get_value() else {
                        continue;
                    };
                    // Go: strings.EqualFold with an ASCII name (no non-ASCII rune folds to one of its letters).
                    if !name.eq_ignore_ascii_case("effect") {
                        continue;
                    }

                    let Some(module_sym) = get_symbol_of_declaration(tp.checker, sf.as_node())
                    else {
                        continue;
                    };

                    // Look for the YieldableError export
                    let Some(export_sym) =
                        tp.checker.try_get_member_in_module_exports_and_properties(
                            "YieldableError",
                            module_sym,
                        )
                    else {
                        continue;
                    };

                    // Verify this is the Cause module by checking for a Cause export
                    if tp
                        .checker
                        .try_get_member_in_module_exports_and_properties("Cause", module_sym)
                        .is_none()
                    {
                        continue;
                    }

                    let Some(export_sym) = tp.resolve_aliased_symbol(Some(export_sym)) else {
                        continue;
                    };

                    let yieldable_error_type =
                        tp.checker.get_declared_type_of_symbol_exported(export_sym);

                    if tp.checker.is_type_assignable_to(t, yieldable_error_type) {
                        return true;
                    }
                }

                false
            },
        )
    }
}
