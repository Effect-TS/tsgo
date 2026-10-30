#![expect(clippy::unreadable_literal, reason = "diagnostic codes are written as in the Go test")]

use std::sync::Arc;

use ets_etscore::EffectPluginOptions;
use ets_shim::P;
use ets_shim::ast::Diagnostic;
use ets_shim::core::{CompilerOptions, TextRange, Tristate};
use ets_shim::diagnostics::{Category, Key};

use crate::*;

// go test: etsexecutehooks/init_test.go:13 makeDiag e8a3faec9194085f
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

// go test: etsexecutehooks/init_test.go:29 TestFilterDiagnosticsForExitCode 7807fdc048acda61
#[test]
fn test_filter_diagnostics_for_exit_code() {
    let mixed = vec![
        make_diag(377009, Category::Suggestion),
        make_diag(377010, Category::Message),
        make_diag(377011, Category::Warning),
        make_diag(377001, Category::Error),
        make_diag(1002, Category::Error),
        make_diag(2304, Category::Warning),
        make_diag(6133, Category::Suggestion),
        make_diag(9999, Category::Message),
    ];
    let non_effect: &[i32] = &[1002, 2304, 6133, 9999];

    struct Case<'a> {
        name: &'static str,
        opts: Option<EffectPluginOptions>,
        want: &'a [i32],
    }
    for tt in [
        Case {
            name: "nil options",
            opts: None,
            want: &[377009, 377010, 377011, 377001, 1002, 2304, 6133, 9999],
        },
        Case {
            name: "nothing ignored",
            opts: Some(EffectPluginOptions::default()),
            want: &[377009, 377010, 377011, 377001, 1002, 2304, 6133, 9999],
        },
        Case {
            name: "suggestions and messages ignored",
            opts: Some(EffectPluginOptions {
                ignore_effect_suggestions_in_tsc_exit_code: true,
                ..Default::default()
            }),
            want: &[377011, 377001, 1002, 2304, 6133, 9999],
        },
        Case {
            name: "warnings ignored",
            opts: Some(EffectPluginOptions {
                ignore_effect_warnings_in_tsc_exit_code: true,
                ..Default::default()
            }),
            want: &[377009, 377010, 377001, 1002, 2304, 6133, 9999],
        },
        Case {
            name: "errors ignored",
            opts: Some(EffectPluginOptions {
                ignore_effect_errors_in_tsc_exit_code: true,
                ..Default::default()
            }),
            want: &[377009, 377010, 377011, 1002, 2304, 6133, 9999],
        },
        Case {
            name: "everything ignored",
            opts: Some(EffectPluginOptions {
                ignore_effect_suggestions_in_tsc_exit_code: true,
                ignore_effect_warnings_in_tsc_exit_code: true,
                ignore_effect_errors_in_tsc_exit_code: true,
                ..Default::default()
            }),
            want: non_effect,
        },
    ] {
        let got: Vec<i32> = filter_diagnostics_for_exit_code_exported(tt.opts.as_ref(), &mixed)
            .iter()
            .map(|diag| diag.code())
            .collect();
        assert_eq!(got, tt.want, "{}: kept codes = {got:?}, want {:?}", tt.name, tt.want);
    }
}

// go test: etsexecutehooks/init_test.go:90 TestNoEmitOnErrorHookFiltersCompilerOptions 65a50fef23bc442d
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

// go test: etsexecutehooks/init_test.go:73 TestFilterDiagnosticsForExitCode_EmptySlice 9e95eae5eeb69e9e
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
