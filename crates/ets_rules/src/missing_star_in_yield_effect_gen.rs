use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{EffectContextFlags, TypeParser};

// go: internal/rules/missing_star_in_yield_effect_gen.go:15 MissingStarInYieldEffectGen
// MissingStarInYieldEffectGen detects bare yield (without *) inside Effect generator scopes.
pub static MISSING_STAR_IN_YIELD_EFFECT_GEN: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "missingStarInYieldEffectGen",
        group: "correctness",
        description: "Detects bare yield (without *) inside Effect generator scopes",
        default_severity: Severity::Error,
        supported_effect: vec!["v3", "v4"],
        codes: vec![tsdiag::INSIDE_THIS_EFFECT_GENERATOR_EFFECT_MISSING_STAR_IN_YIELD_EFFECT_GEN.code(), tsdiag::THIS_USES_YIELD_FOR_AN_EFFECT_VALUE_YIELD_ASTERISK_IS_THE_EFFECT_AWARE_FORM_IN_THIS_CONTEXT_EFFECT_MISSING_STAR_IN_YIELD_EFFECT_GEN.code()],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            let matches = analyze_missing_star_in_yield_effect_gen(ctx.type_parser, ctx.source_file);
            let mut diags = Vec::with_capacity(matches.len());
            for m in &matches {
                let related_info = ctx.new_diagnostic(m.source_file, ctx.get_error_range(m.gen_fn_node), &tsdiag::INSIDE_THIS_EFFECT_GENERATOR_EFFECT_MISSING_STAR_IN_YIELD_EFFECT_GEN, &[], &[]);
                diags.push(ctx.new_diagnostic(m.source_file, m.location, &tsdiag::THIS_USES_YIELD_FOR_AN_EFFECT_VALUE_YIELD_ASTERISK_IS_THE_EFFECT_AWARE_FORM_IN_THIS_CONTEXT_EFFECT_MISSING_STAR_IN_YIELD_EFFECT_GEN, &[related_info], &[]));
            }
            diags
        },
    }
});

// go: internal/rules/missing_star_in_yield_effect_gen.go:35 MissingStarInYieldEffectGenMatch
// MissingStarInYieldEffectGenMatch holds the AST node needed by both the diagnostic rule
// and the quick-fix for the missingStarInYieldEffectGen pattern.
#[derive(Clone)]
pub struct MissingStarInYieldEffectGenMatch {
    pub source_file: P<SourceFile>, // The source file where the diagnostic should be reported
    pub location: TextRange,        // The pre-computed error range for this match
    pub yield_node: P<Node>,        // The yield expression node (for fix range)
    pub gen_fn_node: P<Node>,       // The generator function node (for related info)
}

// go: internal/rules/missing_star_in_yield_effect_gen.go:44 AnalyzeMissingStarInYieldEffectGen
// AnalyzeMissingStarInYieldEffectGen finds all yield expressions inside Effect generators
// that are missing the asterisk (yield instead of yield*).
//
// Rust: Go's (ignored) checker argument is dropped.
pub fn analyze_missing_star_in_yield_effect_gen(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<MissingStarInYieldEffectGenMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<MissingStarInYieldEffectGenMatch>,
    ) -> bool {
        if n.kind() == Kind::YieldExpression {
            let yield_ = n.as_yield_expression();
            if yield_.expression.is_some()
                && yield_.asterisk_token.is_none()
                && tp.get_effect_context_flags(n).intersects(EffectContextFlags::CanYieldEffect)
                && let Some(gen_fn) = tp.get_effect_yield_generator_function(n)
            {
                matches.push(MissingStarInYieldEffectGenMatch {
                    source_file: sf,
                    location: ets_shim::scanner::get_error_range_for_node(sf, n),
                    yield_node: n,
                    gen_fn_node: gen_fn,
                });
            }
        }

        n.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);

    matches
}
