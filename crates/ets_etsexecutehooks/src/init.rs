use std::sync::Arc;

use ets_etscore::EffectPluginOptions;
use ets_shim::P;
use ets_shim::ast::Diagnostic;
use ets_shim::core::CompilerOptions;
use ets_shim::diagnostics;

// go: etsexecutehooks/init.go:13 init 31a1985b41c0da7c
pub(crate) fn init() {
    ets_shim::execute::tsc::register_filter_diagnostics_for_exit_code_callback(Arc::new(
        filter_diagnostics_for_exit_code,
    ));
    ets_shim::compiler::register_filter_diagnostics_for_no_emit_on_error_callback(Arc::new(
        filter_diagnostics_for_exit_code,
    ));
}

// go: etsexecutehooks/init.go:21 filterDiagnosticsForExitCode 86a1b16c97b67195
// filterDiagnosticsForExitCode is the callback registered with the tsc package.
// It extracts the Effect plugin options from compiler options and delegates to
// FilterDiagnosticsForExitCode.
pub(crate) fn filter_diagnostics_for_exit_code(
    opts: &CompilerOptions,
    diagnostics: &[P<Diagnostic>],
) -> Vec<P<Diagnostic>> {
    let effect = opts.effect.get();
    let Some(effect) = effect.as_deref() else {
        return filter_diagnostics_for_exit_code_exported(None, diagnostics);
    };
    filter_diagnostics_for_exit_code_exported(Some(effect), diagnostics)
}

// go: etsexecutehooks/init.go:32 FilterDiagnosticsForExitCode 48a6d605c9e7434a
// FilterDiagnosticsForExitCode filters out Effect diagnostics whose category should
// be ignored for exit-code determination, based on the exit-code config options.
// Non-Effect diagnostics are always retained. If opts is nil, the original slice is
// returned unchanged.
//
// Rust: `_exported` tells it apart from the callback `filterDiagnosticsForExitCode` (docs/PORTING.md, "Naming");
// the unchanged slice is a copy of the handles.
pub fn filter_diagnostics_for_exit_code_exported(
    opts: Option<&EffectPluginOptions>,
    diags: &[P<Diagnostic>],
) -> Vec<P<Diagnostic>> {
    let Some(opts) = opts else {
        return diags.to_vec();
    };

    let mut filtered = Vec::new();
    for &diag in diags {
        if ets_rule::is_effect_code(diag.code())
            && should_ignore_for_exit_code(opts, diag.category())
        {
            continue;
        }
        filtered.push(diag);
    }
    filtered
}

// go: etsexecutehooks/init.go:49 shouldIgnoreForExitCode b5b2ea54f5e836e7
// shouldIgnoreForExitCode returns true if the given diagnostic category should be
// excluded from exit-code determination based on the Effect plugin options.
pub(crate) fn should_ignore_for_exit_code(
    opts: &EffectPluginOptions,
    category: diagnostics::Category,
) -> bool {
    match category {
        diagnostics::Category::Suggestion | diagnostics::Category::Message => {
            opts.ignore_effect_suggestions_in_tsc_exit_code
        }
        diagnostics::Category::Warning => opts.ignore_effect_warnings_in_tsc_exit_code,
        diagnostics::Category::Error => opts.ignore_effect_errors_in_tsc_exit_code,
    }
}
