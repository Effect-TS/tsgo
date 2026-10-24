// Package etscheckerhooks provides Effect diagnostics integration with TypeScript-Go.
// This package registers hooks into the checker to run Effect-specific diagnostics
// after each source file is type checked.

use std::sync::Arc;

use ets_etscore::EffectPluginOptions;
use ets_shim::P;
use ets_shim::ast::SourceFile;
use ets_shim::checker::{Checker, Program};
use ets_shim::core::context::Context;

// go: etscheckerhooks/init.go:20 init
// init registers the Effect diagnostics callbacks with TypeScript-Go.
pub(crate) fn init() {
    // Set the version suffix so that core.Version() includes the Effect version
    ets_shim::core::set_version_suffix(&format!("+effect-tsgo.{}", ets_etscore::EFFECT_VERSION));
    ets_effectconfigraw::register();
    ets_effectconfigcheck::register();
    // Register the after check source file callback
    ets_shim::checker::register_after_check_source_file_callback(Arc::new(after_check_source_file));
    ets_shim::checker::register_expression_checked_callback(
        ets_typeparser::record_checked_expression_type,
    );
}

// go: etscheckerhooks/init.go:32 getEffectConfig
// getEffectConfig retrieves the Effect plugin configuration from the program's compiler options.
// Returns nil if no Effect config is present.
pub(crate) fn get_effect_config(p: &dyn Program) -> Option<Arc<EffectPluginOptions>> {
    p.options().effect.get()
}

// go: etscheckerhooks/init.go:38 afterCheckSourceFile
// afterCheckSourceFile is called after type checking each source file.
// It runs Effect diagnostics if the plugin is enabled.
pub(crate) fn after_check_source_file(
    ctx: &Context,
    program: &'static dyn Program,
    c: &mut Checker,
    sf: P<SourceFile>,
) {
    let effect_config = get_effect_config(program);
    // Rule queries must not overwrite the types the check recorded.
    // Rust: Go's `defer` is a drop guard, so the pause also ends on a panic.
    let _resume = Defer(Some(ets_typeparser::pause_checked_expression_types(c)));
    let Ok(diagnostics) = ets_rulerunner::run(
        ctx,
        program,
        c,
        sf,
        effect_config.as_deref(),
        None,
        ets_rulerunner::min_visible_severity(effect_config.as_deref()),
    ) else {
        return;
    };
    for diag in diagnostics {
        c.add_diagnostic_exported(diag);
    }
}

/// Rust-only: Go's `defer f()`; runs `f` when dropped.
struct Defer<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}
