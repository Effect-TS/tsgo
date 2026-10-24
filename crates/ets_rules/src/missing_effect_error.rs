// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Node, SourceFile};
use ets_shim::checker::{Checker, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::TypeParser;

// go: internal/rules/missing_effect_error.go:20 MissingEffectError
// MissingEffectError detects when an Effect has error types that are not
// handled by the expected type. This happens when assigning an Effect with errors
// to a variable/parameter expecting an Effect with fewer or no errors.
pub static MISSING_EFFECT_ERROR: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "missingEffectError",
    group: "correctness",
    description: "Detects Effect values with unhandled error types",
    default_severity: Severity::Error,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        tsdiag::Missing_errors_0_in_the_expected_Effect_type_effect_missingEffectError.code(),
    ],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let matches = analyze_missing_effect_error(ctx.type_parser, ctx.source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                &tsdiag::Missing_errors_0_in_the_expected_Effect_type_effect_missingEffectError,
                &[],
                &[&m.error_type_str],
            ));
        }
        diags
    },
});

// go: internal/rules/missing_effect_error.go:39 MissingEffectErrorMatch
// MissingEffectErrorMatch holds the analysis results needed by both the
// diagnostic rule and the quick-fixes for the missingEffectError pattern.
#[derive(Clone)]
pub struct MissingEffectErrorMatch {
    pub source_file: P<SourceFile>, // The source file where the diagnostic should be reported
    pub location: TextRange,        // The diagnostic error range
    pub error_node: P<Node>,        // The AST node where the type error occurs
    pub unhandled_errors: Vec<P<Type>>, // The individual error types not handled by the target
    pub expected_error_type: P<Type>, // The target Effect's error type (.E)
    pub error_type_str: String,     // The formatted union string of unhandled errors
}

// go: internal/rules/missing_effect_error.go:50 AnalyzeMissingEffectError
// AnalyzeMissingEffectError finds all relation errors where an Effect has error
// types that are not handled by the expected type.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_missing_effect_error(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<MissingEffectErrorMatch> {
    let mut matches = Vec::new();

    for re in tp.checker().get_relation_errors(sf) {
        // Parse both types as Effects
        let src_effect = tp.effect_type(re.source);
        let tgt_effect = tp.effect_type(re.target);

        // Both must be Effect types
        let (Some(src_effect), Some(tgt_effect)) = (src_effect, tgt_effect) else {
            continue;
        };

        // If source has no errors, nothing to report
        if src_effect.e().flags().intersects(TypeFlags::Never) {
            continue;
        }

        // Find unhandled error types
        let unhandled_errors = find_unhandled_errors(tp, src_effect.e(), tgt_effect.e());
        if !unhandled_errors.is_empty() {
            let error_type_str = format_error_types(tp.checker(), &unhandled_errors);
            matches.push(MissingEffectErrorMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, re.error_node),
                error_node: re.error_node,
                unhandled_errors,
                expected_error_type: tgt_effect.e(),
                error_type_str,
            });
        }
    }

    matches
}

// go: internal/rules/missing_effect_error.go:86 findUnhandledErrors
// findUnhandledErrors returns the source error types that are not assignable to the target error type.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn find_unhandled_errors(
    tp: &mut TypeParser<'_>,
    src_e: P<Type>,
    tgt_e: P<Type>,
) -> Vec<P<Type>> {
    // Unroll source error union into individual members
    let src_members = tp.unroll_union_members(src_e).unwrap_or_default();

    let mut unhandled = Vec::new();
    for member in src_members {
        // Check if this specific member is assignable to target
        if !tp.checker().is_type_assignable_to(member, tgt_e) {
            unhandled.push(member);
        }
    }
    unhandled
}

// go: internal/rules/missing_effect_error.go:101 formatErrorTypes
// formatErrorTypes formats a slice of error types as a union string (e.g., "ErrorA | ErrorB").
pub(crate) fn format_error_types(c: &mut Checker, types: &[P<Type>]) -> String {
    if types.is_empty() {
        return String::new();
    }
    if types.len() == 1 {
        return c.type_to_string_exported(types[0]);
    }
    let mut result = String::new();
    result.push_str(&c.type_to_string_exported(types[0]));
    for &t in &types[1..] {
        result.push_str(" | ");
        result.push_str(&c.type_to_string_exported(t));
    }
    result
}
