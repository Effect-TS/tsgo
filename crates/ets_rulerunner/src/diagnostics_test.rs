use ets_etscore::{EffectPluginOptions, Severity};

use crate::*;

/// Rust-only: Go's `defer etscore.EnterCommandLineMode()()`.
struct CommandLineMode<F: Fn()>(F);

impl<F: Fn()> Drop for CommandLineMode<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

// go test: internal/rulerunner/diagnostics_test.go:9 TestMinVisibleSeverity afccb1403560fb8c
// Rust: the subtests run in order in one test (no other test of this crate touches the mode).
#[test]
fn test_min_visible_severity() {
    struct Case {
        name: &'static str,
        cli: bool,
        options: Option<EffectPluginOptions>,
        want: Severity,
    }
    let cases = [
        Case {
            name: "outside CLI mode every severity is visible",
            cli: false,
            options: Some(EffectPluginOptions {
                include_suggestions_in_tsc: false,
                ..Default::default()
            }),
            want: Severity::Message,
        },
        Case {
            name: "CLI mode without includeSuggestionsInTsc only surfaces warnings and errors",
            cli: true,
            options: Some(EffectPluginOptions {
                include_suggestions_in_tsc: false,
                ..Default::default()
            }),
            want: Severity::Warning,
        },
        Case {
            name: "CLI mode with includeSuggestionsInTsc keeps every severity",
            cli: true,
            options: Some(EffectPluginOptions {
                include_suggestions_in_tsc: true,
                ..Default::default()
            }),
            want: Severity::Message,
        },
        Case {
            name: "CLI mode with nil config keeps every severity",
            cli: true,
            options: None,
            want: Severity::Message,
        },
    ];
    for tc in &cases {
        let _restore = tc.cli.then(|| CommandLineMode(ets_etscore::enter_command_line_mode()));
        let got = min_visible_severity(tc.options.as_ref());
        assert!(got == tc.want, "{}: MinVisibleSeverity = {got}, want {}", tc.name, tc.want);
    }
}
