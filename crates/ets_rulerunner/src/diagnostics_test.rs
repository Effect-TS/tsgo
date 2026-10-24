use ets_etscore::{EffectPluginOptions, Severity};

use crate::*;

/// Rust-only: Go's `restore := etscore.EnterCommandLineMode(); defer restore()`.
struct CommandLineMode<F: Fn()>(F);

impl<F: Fn()> Drop for CommandLineMode<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

// go test: internal/rulerunner/diagnostics_test.go:10 TestMinVisibleSeverity
// Not parallel: EnterCommandLineMode toggles process-global state.
//
// Rust: the subtests run in order in one test (no other test of this crate touches the mode).
#[test]
fn test_min_visible_severity() {
    let include_suggestions =
        EffectPluginOptions { include_suggestions_in_tsc: true, ..Default::default() };
    let drop_suggestions =
        EffectPluginOptions { include_suggestions_in_tsc: false, ..Default::default() };

    // outside CLI mode every severity is visible
    {
        let got = min_visible_severity(Some(&drop_suggestions));
        assert!(got == Severity::Message, "MinVisibleSeverity = {got}, want {}", Severity::Message);
    }

    // CLI mode without includeSuggestionsInTsc only surfaces warnings and errors
    {
        let _restore = CommandLineMode(ets_etscore::enter_command_line_mode());
        let got = min_visible_severity(Some(&drop_suggestions));
        assert!(got == Severity::Warning, "MinVisibleSeverity = {got}, want {}", Severity::Warning);
    }

    // CLI mode with includeSuggestionsInTsc keeps every severity
    {
        let _restore = CommandLineMode(ets_etscore::enter_command_line_mode());
        let got = min_visible_severity(Some(&include_suggestions));
        assert!(got == Severity::Message, "MinVisibleSeverity = {got}, want {}", Severity::Message);
    }

    // CLI mode with nil config keeps every severity
    {
        let _restore = CommandLineMode(ets_etscore::enter_command_line_mode());
        let got = min_visible_severity(None);
        assert!(got == Severity::Message, "MinVisibleSeverity = {got}, want {}", Severity::Message);
    }
}
