// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::Diagnostic;
use ets_shim::checker::{Checker, Type};
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::TypeParser;

// go: internal/rules/missing_layer_context.go:19 MissingLayerContext 43e23eee92e5390b
// MissingLayerContext detects when a Layer has context requirements that are not
// handled by the expected type. This happens when assigning a Layer with requirements
// to a variable/parameter expecting a Layer with fewer or no requirements.
pub static MISSING_LAYER_CONTEXT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "missingLayerContext",
    group: "correctness",
    description: "Detects Layer values with unhandled context requirements",
    default_severity: Severity::Error,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        tsdiag::MISSING_0_IN_THE_EXPECTED_LAYER_CONTEXT_EFFECT_MISSING_LAYER_CONTEXT.code(),
    ],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        let source_file = ctx.source_file;
        for re in ctx.checker().get_relation_errors(source_file) {
            // Parse both types as Layers
            let src_layer = ctx.type_parser.layer_type(re.source);
            let tgt_layer = ctx.type_parser.layer_type(re.target);

            // Both must be Layer types
            let (Some(src_layer), Some(tgt_layer)) = (src_layer, tgt_layer) else {
                continue;
            };

            // Find unhandled context types by checking each source RIn member
            // against the target RIn type
            let mut unhandled =
                find_unhandled_layer_contexts(ctx.type_parser, src_layer.r_in(), tgt_layer.r_in());
            if !unhandled.is_empty() {
                // Sort deterministically by type name (alphabetical)
                let c = ctx.checker();
                ets_go::sort::slice(&mut unhandled, |a, b| {
                    c.type_to_string_exported(*a) < c.type_to_string_exported(*b)
                });
                let context_type_str = format_layer_context_types(ctx.checker(), &unhandled);
                let diag = ctx.new_diagnostic(
                    ctx.source_file,
                    ctx.get_error_range(re.error_node),
                    &tsdiag::MISSING_0_IN_THE_EXPECTED_LAYER_CONTEXT_EFFECT_MISSING_LAYER_CONTEXT,
                    &[],
                    &[&context_type_str],
                );
                diags.push(diag);
            }
        }

        diags
    },
});

// go: internal/rules/missing_layer_context.go:58 findUnhandledLayerContexts dcd5895da9bc4e5c
// findUnhandledLayerContexts returns the source Layer RIn types that are not assignable to the target RIn type.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn find_unhandled_layer_contexts(
    tp: &mut TypeParser<'_>,
    src_r_in: P<Type>,
    tgt_r_in: P<Type>,
) -> Vec<P<Type>> {
    // Unroll source RIn union into individual members
    let src_members = tp.unroll_union_members(src_r_in).unwrap_or_default();

    let mut unhandled = Vec::new();
    for member in src_members {
        // Check if this specific member is assignable to target
        if !tp.checker().is_type_assignable_to(member, tgt_r_in) {
            unhandled.push(member);
        }
    }
    unhandled
}

// go: internal/rules/missing_layer_context.go:73 formatLayerContextTypes 038ebc4aaa7052bd
// formatLayerContextTypes formats a slice of Layer context types as a union string (e.g., "ServiceA | ServiceB").
pub(crate) fn format_layer_context_types(c: &mut Checker, types: &[P<Type>]) -> String {
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
