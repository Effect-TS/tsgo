#![expect(clippy::unreadable_literal, reason = "diagnostic codes are written as in the Go test")]

use std::sync::Arc;

use ets_etscore::EffectPluginOptions;
use ets_shim::P;
use ets_shim::ast::Diagnostic;
use ets_shim::core::{CompilerOptions, TextRange, Tristate};
use ets_shim::diagnostics::{Category, Key};

use crate::*;

// go test: etsexecutehooks/init_test.go:13 makeDiag 223cb147aebcc215
// makeDiag creates a minimal diagnostic with the given code and category.
fn make_diag(code: i32, category: Category) -> P<Diagnostic> {
    ets_shim::ast::new_diagnostic_from_serialized(
        None,                 // file
        TextRange::new(0, 0), // loc
        code,                 // code
        category,             // category
        Key(""),              // messageKey
        Vec::new(),           // messageArgs
        &[],                  // messageChain
        &[],                  // relatedInformation
        false,                // reportsUnnecessary
        false,                // reportsDeprecated
        false,                // skippedOnNoEmit
    )
}

fn codes(diags: &[P<Diagnostic>]) -> Vec<i32> {
    diags.iter().map(|d| d.code()).collect()
}

// go test: etsexecutehooks/init_test.go:29 TestFilterDiagnosticsForExitCode_NilOptions d2be1f49fa9e0c27
#[test]
fn test_filter_diagnostics_for_exit_code_nil_options() {
    let diags = vec![
        make_diag(377009, Category::Suggestion),
        make_diag(377001, Category::Error),
        make_diag(1002, Category::Error),
    ];
    let result = filter_diagnostics_for_exit_code_exported(None, &diags);
    assert_eq!(
        result.len(),
        diags.len(),
        "expected {} diagnostics, got {}",
        diags.len(),
        result.len()
    );
    // Verify it's the same slice (no filtering)
    for i in 0..diags.len() {
        assert!(result[i] == diags[i], "expected same diagnostic at index {i}");
    }
}

// go test: etsexecutehooks/init_test.go:48 TestFilterDiagnosticsForExitCode_DefaultConfig ef937f2765c01db4
#[test]
fn test_filter_diagnostics_for_exit_code_default_config() {
    // Default: suggestions ignored, warnings not ignored
    let opts = EffectPluginOptions {
        ignore_effect_suggestions_in_tsc_exit_code: true,
        ignore_effect_warnings_in_tsc_exit_code: false,
        ..Default::default()
    };

    let diags = vec![
        make_diag(377009, Category::Suggestion), // Effect suggestion — should be filtered
        make_diag(377011, Category::Warning),    // Effect warning — should be kept
        make_diag(377001, Category::Error),      // Effect error — should be kept
        make_diag(1002, Category::Error),        // Non-Effect error — should be kept
        make_diag(2304, Category::Suggestion),   // Non-Effect suggestion — should be kept
    ];

    let result = filter_diagnostics_for_exit_code_exported(Some(&opts), &diags);

    assert_eq!(result.len(), 4, "expected 4 diagnostics, got {}", result.len());
    // Verify the correct diagnostics were kept
    assert_eq!(codes(&result), [377011, 377001, 1002, 2304]);
}

// go test: etsexecutehooks/init_test.go:84 TestFilterDiagnosticsForExitCode_IgnoreWarnings 8952d2c5ef4a7076
#[test]
fn test_filter_diagnostics_for_exit_code_ignore_warnings() {
    let opts = EffectPluginOptions {
        ignore_effect_suggestions_in_tsc_exit_code: true,
        ignore_effect_warnings_in_tsc_exit_code: true,
        ..Default::default()
    };

    let diags = vec![
        make_diag(377009, Category::Suggestion), // Effect suggestion — filtered
        make_diag(377011, Category::Warning),    // Effect warning — filtered
        make_diag(377001, Category::Error),      // Effect error — kept
        make_diag(1002, Category::Error),        // Non-Effect error — kept
        make_diag(2304, Category::Warning),      // Non-Effect warning — kept
    ];

    let result = filter_diagnostics_for_exit_code_exported(Some(&opts), &diags);

    assert_eq!(result.len(), 3, "expected 3 diagnostics, got {}", result.len());
    assert_eq!(codes(&result), [377001, 1002, 2304]);
}

// go test: etsexecutehooks/init_test.go:115 TestNoEmitOnErrorHookFiltersCompilerOptions 65a50fef23bc442d
#[test]
fn test_no_emit_on_error_hook_filters_compiler_options() {
    let opts = CompilerOptions {
        no_emit_on_error: Tristate::True,
        effect: Some(Arc::new(EffectPluginOptions {
            ignore_effect_warnings_in_tsc_exit_code: true,
            ..Default::default()
        }))
        .into(),
        ..Default::default()
    };
    let result =
        init::filter_diagnostics_for_exit_code(&opts, &[make_diag(377011, Category::Warning)]);
    assert!(result.is_empty(), "expected ignored Effect warning to be filtered");

    let result = init::filter_diagnostics_for_exit_code(&opts, &[make_diag(1002, Category::Error)]);
    assert_eq!(result.len(), 1, "expected TypeScript error to be retained");
}

// go test: etsexecutehooks/init_test.go:132 TestFilterDiagnosticsForExitCode_ErrorsNotFilteredByDefault 4c54fa79c44e1cac
#[test]
fn test_filter_diagnostics_for_exit_code_errors_not_filtered_by_default() {
    // With suggestion+warning ignore set but error ignore NOT set, errors are kept
    let opts = EffectPluginOptions {
        ignore_effect_suggestions_in_tsc_exit_code: true,
        ignore_effect_warnings_in_tsc_exit_code: true,
        ..Default::default()
    };

    let diags = vec![
        make_diag(377001, Category::Error), // Effect error — kept
        make_diag(377003, Category::Error), // Effect error — kept
        make_diag(377004, Category::Error), // Effect error — kept
    ];

    let result = filter_diagnostics_for_exit_code_exported(Some(&opts), &diags);

    assert_eq!(result.len(), 3, "expected 3 diagnostics, got {}", result.len());
    for (i, diag) in result.iter().enumerate() {
        assert!(*diag == diags[i], "expected same diagnostic at index {i}");
    }
}

// go test: etsexecutehooks/init_test.go:158 TestFilterDiagnosticsForExitCode_IgnoreErrors 7112c2f17cc1481c
#[test]
fn test_filter_diagnostics_for_exit_code_ignore_errors() {
    // When IgnoreEffectErrorsInTscExitCode is true, Effect errors are filtered
    let opts =
        EffectPluginOptions { ignore_effect_errors_in_tsc_exit_code: true, ..Default::default() };

    let diags = vec![
        make_diag(377001, Category::Error), // Effect error — filtered
        make_diag(377003, Category::Error), // Effect error — filtered
        make_diag(1002, Category::Error),   // Non-Effect error — kept
    ];

    let result = filter_diagnostics_for_exit_code_exported(Some(&opts), &diags);

    assert_eq!(result.len(), 1, "expected 1 diagnostic, got {}", result.len());
    assert_eq!(result[0].code(), 1002, "expected code 1002, got {}", result[0].code());
}

// go test: etsexecutehooks/init_test.go:181 TestFilterDiagnosticsForExitCode_IgnoreErrorsFalse 98605099e00d9264
#[test]
fn test_filter_diagnostics_for_exit_code_ignore_errors_false() {
    // When IgnoreEffectErrorsInTscExitCode is false (default), Effect errors are kept
    let opts =
        EffectPluginOptions { ignore_effect_errors_in_tsc_exit_code: false, ..Default::default() };

    let diags = vec![
        make_diag(377001, Category::Error), // Effect error — kept
        make_diag(377003, Category::Error), // Effect error — kept
        make_diag(1002, Category::Error),   // Non-Effect error — kept
    ];

    let result = filter_diagnostics_for_exit_code_exported(Some(&opts), &diags);

    assert_eq!(result.len(), 3, "expected 3 diagnostics, got {}", result.len());
}

// go test: etsexecutehooks/init_test.go:201 TestFilterDiagnosticsForExitCode_NonEffectErrorsNeverFiltered 5666aec8ed3d11fc
#[test]
fn test_filter_diagnostics_for_exit_code_non_effect_errors_never_filtered() {
    // Non-Effect errors are never filtered regardless of option
    let opts = EffectPluginOptions {
        ignore_effect_errors_in_tsc_exit_code: true,
        ignore_effect_suggestions_in_tsc_exit_code: true,
        ignore_effect_warnings_in_tsc_exit_code: true,
        ..Default::default()
    };

    let diags = vec![
        make_diag(1002, Category::Error),      // Non-Effect error — kept
        make_diag(2304, Category::Warning),    // Non-Effect warning — kept
        make_diag(6133, Category::Suggestion), // Non-Effect suggestion — kept
    ];

    let result = filter_diagnostics_for_exit_code_exported(Some(&opts), &diags);

    assert_eq!(result.len(), 3, "expected 3 diagnostics, got {}", result.len());
    for (i, diag) in result.iter().enumerate() {
        assert!(*diag == diags[i], "expected same diagnostic at index {i}");
    }
}

// go test: etsexecutehooks/init_test.go:228 TestFilterDiagnosticsForExitCode_NonEffectNeverFiltered 5177ab42102b1574
#[test]
fn test_filter_diagnostics_for_exit_code_non_effect_never_filtered() {
    // Non-Effect diagnostics of any category are never filtered
    let opts = EffectPluginOptions {
        ignore_effect_suggestions_in_tsc_exit_code: true,
        ignore_effect_warnings_in_tsc_exit_code: true,
        ..Default::default()
    };

    let diags = vec![
        make_diag(1002, Category::Error),
        make_diag(2304, Category::Warning),
        make_diag(6133, Category::Suggestion),
        make_diag(9999, Category::Message),
    ];

    let result = filter_diagnostics_for_exit_code_exported(Some(&opts), &diags);

    assert_eq!(result.len(), 4, "expected 4 diagnostics, got {}", result.len());
    for (i, diag) in result.iter().enumerate() {
        assert!(*diag == diags[i], "expected same diagnostic at index {i}");
    }
}

// go test: etsexecutehooks/init_test.go:255 TestFilterDiagnosticsForExitCode_BothFalse 9b29604dd5f75e8f
#[test]
fn test_filter_diagnostics_for_exit_code_both_false() {
    // When both ignore options are false, nothing is filtered
    let opts = EffectPluginOptions {
        ignore_effect_suggestions_in_tsc_exit_code: false,
        ignore_effect_warnings_in_tsc_exit_code: false,
        ..Default::default()
    };

    let diags = vec![
        make_diag(377009, Category::Suggestion),
        make_diag(377011, Category::Warning),
        make_diag(377001, Category::Error),
    ];

    let result = filter_diagnostics_for_exit_code_exported(Some(&opts), &diags);

    assert_eq!(result.len(), 3, "expected 3 diagnostics, got {}", result.len());
}

// go test: etsexecutehooks/init_test.go:276 TestFilterDiagnosticsForExitCode_EmptySlice 9e95eae5eeb69e9e
// Rust: Go's nil and empty input slices are both an empty slice, and both give an empty result.
#[test]
fn test_filter_diagnostics_for_exit_code_empty_slice() {
    let opts = EffectPluginOptions {
        ignore_effect_suggestions_in_tsc_exit_code: true,
        ..Default::default()
    };

    let result = filter_diagnostics_for_exit_code_exported(Some(&opts), &[]);
    assert!(result.is_empty(), "expected empty slice, got {} diagnostics", result.len());
}

// go test: etsexecutehooks/init_test.go:293 TestFilterDiagnosticsForExitCode_MessageCategory 8e1f343a3404e6b6
#[test]
fn test_filter_diagnostics_for_exit_code_message_category() {
    // Message category follows the same config as Suggestion
    let opts = EffectPluginOptions {
        ignore_effect_suggestions_in_tsc_exit_code: true,
        ..Default::default()
    };

    let diags = vec![
        make_diag(377009, Category::Message), // Effect message — filtered (same config as suggestion)
    ];

    let result = filter_diagnostics_for_exit_code_exported(Some(&opts), &diags);

    assert!(result.is_empty(), "expected 0 diagnostics, got {}", result.len());
}
