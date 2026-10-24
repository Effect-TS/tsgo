use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{EffectContextFlags, TypeParser};

// go: internal/rules/nested_effect_gen_yield.go:16 NestedEffectGenYield
// NestedEffectGenYield warns when yield* targets a bare nested Effect.gen inside
// an existing Effect generator context, since the inner generator can usually be
// inlined directly into the surrounding Effect generator.
pub static NESTED_EFFECT_GEN_YIELD: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "nestedEffectGenYield",
        group: "style",
        description: "Warns when yielding a nested bare Effect.gen inside an existing Effect generator context",
        default_severity: Severity::Off,
        supported_effect: vec!["v3", "v4"],
        codes: vec![
            tsdiag::This_yield_Asterisk_is_applied_to_a_nested_Effect_gen_that_can_be_inlined_in_the_parent_Effect_generator_context_effect_nestedEffectGenYield.code(),
        ],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            let matches = analyze_nested_effect_gen_yield(ctx.type_parser, ctx.source_file);
            let mut diags = Vec::with_capacity(matches.len());
            for m in &matches {
                diags.push(ctx.new_diagnostic(
                    ctx.source_file,
                    m.location,
                    &tsdiag::This_yield_Asterisk_is_applied_to_a_nested_Effect_gen_that_can_be_inlined_in_the_parent_Effect_generator_context_effect_nestedEffectGenYield,
                    &[],
                    &[],
                ));
            }
            diags
        },
    }
});

// go: internal/rules/nested_effect_gen_yield.go:40 NestedEffectGenYieldMatch
#[derive(Clone, Copy)]
pub struct NestedEffectGenYieldMatch {
    pub location: TextRange,
}

// go: internal/rules/nested_effect_gen_yield.go:44 AnalyzeNestedEffectGenYield
pub fn analyze_nested_effect_gen_yield(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<NestedEffectGenYieldMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<NestedEffectGenYieldMatch>,
    ) -> bool {
        if node.kind() == Kind::YieldExpression {
            let yield_expr = node.as_yield_expression();
            if yield_expr.asterisk_token.is_some()
                && let Some(expression) = yield_expr.expression
                && tp.get_effect_context_flags(node).intersects(EffectContextFlags::CanYieldEffect)
                && tp.effect_gen_call(expression).is_some()
            {
                matches.push(NestedEffectGenYieldMatch {
                    location: ets_shim::scanner::get_error_range_for_node(sf, expression),
                });
            }
        }

        node.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}
