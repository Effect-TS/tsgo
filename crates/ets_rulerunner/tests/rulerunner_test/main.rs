// Port of internal/rulerunner's external test package (`package rulerunner_test`): one module per Go file.
//
// Rust: the Go package imports `etscheckerhooks` for its `init`, which registers the after-check hooks that run
// the rules: the helpers call `ets_etscheckerhooks::init()` before building a program.

mod api_stability_leak_inheritance_test;
mod api_stability_leak_internal_properties_test;
mod api_stability_leak_test;
mod fail_cause_to_constructor_test;
mod flat_map_identity_to_flatten_test;
mod manual_metric_tap_to_track_test;
mod map_map_error_to_map_both_test;
mod prefer_effect_array_test;
mod prefer_effect_record_test;
mod prefer_effect_string_test;
mod provide_merge_all_to_array_test;
mod type_union_to_schema_union_test;

use std::sync::Arc;

use ets_go::testing::T;
use ets_shim::compiler::{Program, ProgramOptions, ProgramRegions};

/// Rust-only: `compiler.NewProgram` for a test, freed when the test ends (Go: the GC). The program's checkers and
/// what its file loader allocates go to regions, and a cleanup frees the program with them when the test's body has
/// returned (`docs/TSRS_API.md`, "The API path"): the tests build hundreds of programs, which would otherwise stay
/// alive until the process exits.
pub(crate) fn new_test_program(t: &T, mut program_options: ProgramOptions) -> &'static Program {
    program_options.checker_regions = true;
    program_options.checker_diagnostic_regions = true;
    let loader_regions = Arc::new(ProgramRegions::default());
    program_options.regions = Some(Arc::clone(&loader_regions));
    let program = ets_shim::compiler::new_program(program_options);
    t.cleanup(move || {
        // SAFETY: a full build from `new_program`, never the source or result of `update_program`; cleanups run
        // after the test's body returned, so its checker handles and everything it read from the program are gone
        // (the tests keep strings).
        unsafe { ets_shim::compiler::free_unshared_program(program) };
        // Last: the program pointed into them.
        drop(loader_regions);
    });
    program
}
