#![expect(clippy::unreadable_literal, reason = "diagnostic codes are written as in the Go test")]

use std::cell::RefCell;

use ets_etscore::{EffectPluginOptions, Severity};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, SourceFileParseOptions};
use ets_shim::core::context::Context;
use ets_shim::core::{ScriptKind, TextRange};
use ets_shim::diagnostics::{Category, Key};
use rustc_hash::FxHashMap;

use crate::*;

// go test: etsoxlintrunner/runner_test.go:15 TestRunRuleAndReportRequiresAdapterCallbacks 0fd1efb5ce757261
#[test]
fn test_run_rule_and_report_requires_adapter_callbacks() {
    let ctx = Context::background();
    let err = run_rule_and_report(
        &ctx,
        None,
        None,
        None,
        None,
        "floatingEffect",
        DiagnosticAdapter::default(),
    );
    assert!(
        err.as_ref().is_err_and(|e| e == "diagnostic adapter Message callback is required"),
        "unexpected missing Message callback error: {err:?}"
    );

    let message = |_: P<Diagnostic>| String::new();
    let err = run_rule_and_report(
        &ctx,
        None,
        None,
        None,
        None,
        "floatingEffect",
        DiagnosticAdapter { message: Some(&message), report: None },
    );
    assert!(
        err.as_ref().is_err_and(|e| e == "diagnostic adapter Report callback is required"),
        "unexpected missing Report callback error: {err:?}"
    );
}

// go test: etsoxlintrunner/runner_test.go:31 TestNormalizeOptionsUsesPluginDefaults 7af3ff1ac9996e29
#[test]
fn test_normalize_options_uses_plugin_defaults() {
    let options = normalize_options(None, "floatingEffect");
    assert!(
        options.refactors
            && options.diagnostics
            && options.include_suggestions_in_tsc
            && options.quickinfo
            && options.completions
            && options.goto
            && options.renames
            && options.ignore_effect_suggestions_in_tsc_exit_code,
        "expected nil options to use the name-only plugin defaults"
    );
    let severity = options.diagnostic_severity.as_ref().unwrap();
    assert!(
        severity.get("floatingEffect") == Some(&Severity::Error),
        "unexpected rule severity: {:?}",
        severity.get("floatingEffect")
    );
    assert!(
        severity.get("unusedDirective") == Some(&Severity::Off),
        "unexpected unusedDirective severity: {:?}",
        severity.get("unusedDirective")
    );
}

// go test: etsoxlintrunner/runner_test.go:48 TestNormalizeOptionsDoesNotMutateInput 0a90d55abbb5242a
#[test]
fn test_normalize_options_does_not_mutate_input() {
    let input = EffectPluginOptions {
        pipeable_min_arg_count: 4,
        diagnostic_severity: Some(FxHashMap::from_iter([
            ("floatingEffect".to_string(), Severity::Warning),
            ("unusedDirective".to_string(), Severity::Warning),
        ])),
        ..Default::default()
    };
    let options = normalize_options(Some(&input), "floatingEffect");

    assert!(
        options.pipeable_min_arg_count == 4,
        "unexpected pipeable minimum: {}",
        options.pipeable_min_arg_count
    );
    let severity = options.diagnostic_severity.as_ref().unwrap();
    assert!(
        severity.get("floatingEffect") == Some(&Severity::Error),
        "unexpected normalized rule severity: {:?}",
        severity.get("floatingEffect")
    );
    assert!(
        severity.get("unusedDirective") == Some(&Severity::Off),
        "unexpected normalized unusedDirective severity: {:?}",
        severity.get("unusedDirective")
    );
    let input_severity = input.diagnostic_severity.as_ref().unwrap();
    assert!(
        input_severity.get("floatingEffect") == Some(&Severity::Warning)
            && input_severity.get("unusedDirective") == Some(&Severity::Warning),
        "input diagnostic severities were mutated"
    );
}

/// Rust-only: Go's `ast.NewDiagnosticFromSerialized(file, loc, code, CategoryError, "", nil, nil, related, ...)`.
fn serialized_diagnostic(
    file: Option<P<ets_shim::ast::SourceFile>>,
    loc: TextRange,
    code: i32,
    related: &[P<Diagnostic>],
) -> P<Diagnostic> {
    ets_shim::ast::new_diagnostic_from_serialized(
        file,
        loc,
        code,
        Category::Error,
        Key(""),
        Vec::new(),
        &[],
        related,
        false,
        false,
        false,
    )
}

// go test: etsoxlintrunner/runner_test.go:75 TestReportDiagnostics 6924133922e9201d
//
// Rust: Go checks that the lazy `Suggestions` callback is non-nil; the Rust field is a `Box`, always present.
#[test]
fn test_report_diagnostics() {
    let source_file = ets_shim::parser::parse_source_file(
        SourceFileParseOptions { file_name: "/source.ts".to_string(), ..Default::default() },
        "const effect = 1",
        ScriptKind::TS,
    );
    let related = serialized_diagnostic(Some(source_file), TextRange::new(9, 15), 377007, &[]);
    let diagnostic =
        serialized_diagnostic(Some(source_file), TextRange::new(3, 7), 377001, &[related]);

    let reported: RefCell<Vec<(TextRange, String, String, Vec<ReportedRelatedInformation>)>> =
        RefCell::new(Vec::new());
    let message = |diagnostic: P<Diagnostic>| {
        if diagnostic == related {
            return "Inside this Effect generator. effect(floatingEffect)".to_string();
        }
        "Effect values must be yielded or assigned effect(floatingEffect)".to_string()
    };
    let mut report = |diagnostic: ReportedDiagnostic<'_>| {
        reported.borrow_mut().push((
            diagnostic.range,
            diagnostic.message_id,
            diagnostic.description,
            diagnostic.related_information,
        ));
    };
    report_diagnostics(
        &Context::background(),
        &[diagnostic],
        "floatingEffect",
        None,
        None,
        Some(source_file),
        None,
        DiagnosticAdapter { message: Some(&message), report: Some(&mut report) },
    );

    let reported = reported.into_inner();
    assert!(reported.len() == 1, "expected one reported diagnostic, got {}", reported.len());
    let (range, message_id, description, related_information) = &reported[0];
    assert!(*range == TextRange::new(3, 7), "unexpected range: {range:?}");
    assert!(message_id == "TS377001", "unexpected message ID: {message_id:?}");
    assert!(
        description == "Effect values must be yielded or assigned",
        "unexpected description: {description:?}"
    );
    assert!(
        related_information.len() == 1,
        "expected one related diagnostic, got {}",
        related_information.len()
    );
    let reported_related = &related_information[0];
    assert!(
        reported_related.file_name == "/source.ts",
        "unexpected related filename: {:?}",
        reported_related.file_name
    );
    assert!(
        reported_related.range == TextRange::new(9, 15),
        "unexpected related range: {:?}",
        reported_related.range
    );
    assert!(
        reported_related.description == "Inside this Effect generator.",
        "unexpected related description: {:?}",
        reported_related.description
    );
}

// go test: etsoxlintrunner/runner_test.go:149 TestReportDiagnosticsPreservesNonSuffixRuleText d2805ddd4b663b66
#[test]
fn test_report_diagnostics_preserves_non_suffix_rule_text() {
    let diagnostic = serialized_diagnostic(None, TextRange::new(0, 0), 377001, &[]);

    for message in [
        "effect(floatingEffect) should remain when it is not a suffix",
        "message effect(otherRule)",
        "message effect(floatingEffect) ",
    ] {
        let description = RefCell::new(String::new());
        let message_fn = |_: P<Diagnostic>| message.to_string();
        let mut report = |diagnostic: ReportedDiagnostic<'_>| {
            *description.borrow_mut() = diagnostic.description;
        };
        report_diagnostics(
            &Context::background(),
            &[diagnostic],
            "floatingEffect",
            None,
            None,
            None,
            None,
            DiagnosticAdapter { message: Some(&message_fn), report: Some(&mut report) },
        );

        let description = description.into_inner();
        assert!(description == message, "unexpected description: {description:?}");
    }
}
