//! Rust-only: what the Go runner takes from typescript-go's test harness (`testutil/harnessutil`,
//! `testutil/baseline`), which tsrs has only inside its `tsrs_testrunner` binary, plus the program plumbing Go's GC
//! makes unnecessary and the hook registration Go does with blank imports.

use std::sync::{Arc, Once};

use ets_shim::compiler::{Program, ProgramRegions};
use ets_shim::core::arena::Region;
use ets_shim::core::tspath;
use ets_shim::tsoptions::ParseConfigHost;
use ets_shim::vfs::FS;

/// Rust-only: Go's `harnessutil.TestFile` (the fields the runner uses).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestFile {
    pub unit_name: String,
    pub content: String,
}

/// Rust-only: Go's `harnessutil.GetConfigNameFromFileName` (as `tsrs_testrunner` ports it): the lower-cased base
/// name for a `tsconfig.json` / `jsconfig.json`, else `""`.
pub(crate) fn get_config_name_from_file_name(filename: &str) -> String {
    let basename_lower = tspath::get_base_file_name(filename).to_ascii_lowercase();
    if basename_lower == "tsconfig.json" || basename_lower == "jsconfig.json" {
        return basename_lower;
    }
    String::new()
}

/// Rust-only: Go's `harnessutil` `baseline.DiffText(oldName, newName, expected, actual)`: a unified diff of the
/// reference and the actual baseline, shown when they differ.
pub fn diff_text(old_name: &str, new_name: &str, expected: &str, actual: &str) -> String {
    similar::TextDiff::from_lines(expected, actual)
        .unified_diff()
        .header(old_name, new_name)
        .to_string()
}

/// Rust-only: Go's `tsoptions.NewDefaultParseConfigHost(fs)`.
pub(crate) struct DefaultParseConfigHost {
    pub(crate) fs: Arc<dyn FS>,
    pub(crate) current_directory: String,
}

impl ParseConfigHost for DefaultParseConfigHost {
    fn fs(&self) -> &dyn FS {
        &*self.fs
    }

    fn get_current_directory(&self) -> &str {
        &self.current_directory
    }
}

/// Rust-only: extends a borrow to `'static` for a tsrs API that takes `&'static` but only reads during the call.
///
/// # Safety
/// The callee must not keep the reference beyond the call.
pub(crate) unsafe fn assume_static<T: ?Sized>(value: &T) -> &'static T {
    // SAFETY: the caller guarantees the reference does not outlive the call it is passed to.
    unsafe { &*std::ptr::from_ref(value) }
}

/// Rust-only: registers the Effect hooks, as Go's blank imports of the hook packages do.
///
/// The effecttest sources import them (`runner.go` imports `etscheckerhooks`; the quick-fix, refactor,
/// document-symbol and inlay-hint tests import `etstesthooks` and `etslshooks`), whose `init()`s register the Effect
/// checker, language-service and fourslash callbacks. Rust has no `init()`: each hook crate has a `pub fn init()`
/// (idempotent), called here once, in Go's import order (the binary `ets_tsrs` registers the first two the same
/// way).
pub fn init_hooks() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        ets_etscheckerhooks::init();
        ets_etslshooks::init();
        ets_etstesthooks::init();
    });
}

/// Rust-only: frees a program `run_effect_test` built, with everything it allocated (Go: the GC, after
/// `ReleaseProgram`): the program and its checkers (with their Effect links), its file loader's regions, then the
/// case's own region.
///
/// # Safety
/// Nothing from the program (nodes, types, symbols, diagnostics, checker handles) is used afterwards.
pub(crate) unsafe fn release_test_program(
    program: &'static Program,
    loader_regions: Arc<ProgramRegions>,
    region: Region,
) {
    // SAFETY: a full build from `new_program`, never the source or result of `update_program`, and unused from
    // here on (this function's contract).
    unsafe { ets_shim::compiler::free_unshared_program(program) };
    // Last: the program pointed into them.
    drop(loader_regions);
    drop(region);
}
