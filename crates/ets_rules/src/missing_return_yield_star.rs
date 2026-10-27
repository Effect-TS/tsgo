// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, OuterExpressionKinds, SourceFile};
use ets_shim::checker::TypeFlags;
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{EffectContextFlags, TypeParser};

// go: internal/rules/missing_return_yield_star.go:16 MissingReturnYieldStar 0e64a1c8b866398e
// MissingReturnYieldStar suggests "return yield*" for Effects that never succeed.
pub static MISSING_RETURN_YIELD_STAR: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "missingReturnYieldStar",
        group: "correctness",
        description: "Suggests using return yield* for Effects that never succeed",
        default_severity: Severity::Error,
        supported_effect: vec!["v3", "v4"],
        codes: vec![tsdiag::THIS_EFFECT_NEVER_SUCCEEDS_USING_RETURN_YIELD_ASTERISK_PRESERVES_A_DEFINITIVE_GENERATOR_EXIT_POINT_FOR_TYPE_NARROWING_AND_TOOLING_SUPPORT_EFFECT_MISSING_RETURN_YIELD_STAR.code()],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            let matches = analyze_missing_return_yield_star(ctx.type_parser, ctx.source_file);
            let mut diags = Vec::with_capacity(matches.len());
            for m in &matches {
                diags.push(ctx.new_diagnostic(m.source_file, m.location, &tsdiag::THIS_EFFECT_NEVER_SUCCEEDS_USING_RETURN_YIELD_ASTERISK_PRESERVES_A_DEFINITIVE_GENERATOR_EXIT_POINT_FOR_TYPE_NARROWING_AND_TOOLING_SUPPORT_EFFECT_MISSING_RETURN_YIELD_STAR, &[], &[]));
            }
            diags
        },
    }
});

// go: internal/rules/missing_return_yield_star.go:35 MissingReturnYieldStarMatch 734cc244bf8f7f67
// MissingReturnYieldStarMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the missingReturnYieldStar pattern.
#[derive(Clone)]
pub struct MissingReturnYieldStarMatch {
    pub source_file: P<SourceFile>, // The source file where the diagnostic should be reported
    pub location: TextRange,        // The pre-computed error range for this match
    pub yield_node: P<Node>,        // The yield* expression node (for diagnostic location)
    pub expr_stmt_node: P<Node>,    // The expression statement node (for quickfix replacement)
}

// go: internal/rules/missing_return_yield_star.go:44 AnalyzeMissingReturnYieldStar 302621945792df17
// AnalyzeMissingReturnYieldStar finds all yield* expressions inside Effect generators
// where the yielded Effect never succeeds, suggesting "return yield*" instead.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), so it is dropped.
pub fn analyze_missing_return_yield_star(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<MissingReturnYieldStarMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<MissingReturnYieldStarMatch>,
    ) -> bool {
        if n.kind() == Kind::ExpressionStatement {
            let expr = n.expression().unwrap();
            let unwrapped = ast::skip_outer_expressions(expr, OuterExpressionKinds::All);
            if unwrapped.kind() == Kind::YieldExpression {
                let yield_ = unwrapped.as_yield_expression();
                if yield_.asterisk_token.is_some()
                    && let Some(yield_expression) = yield_.expression
                    && should_report_missing_return_yield_star(
                        tp,
                        Some(n),
                        Some(unwrapped),
                        Some(yield_expression),
                    )
                {
                    matches.push(MissingReturnYieldStarMatch {
                        source_file: sf,
                        location: ets_shim::scanner::get_error_range_for_node(sf, unwrapped),
                        yield_node: unwrapped,
                        expr_stmt_node: n,
                    });
                }
            }
        }

        n.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);

    matches
}

// go: internal/rules/missing_return_yield_star.go:80 shouldReportMissingReturnYieldStar 3c32943237704a26
//
// Rust: Go's checker argument is the parser's checker (never nil), so it is dropped.
pub(crate) fn should_report_missing_return_yield_star(
    tp: &mut TypeParser<'_>,
    expr_stmt_node: Option<P<Node>>,
    yield_node: Option<P<Node>>,
    expr: Option<P<Node>>,
) -> bool {
    let (Some(expr_stmt_node), Some(_), Some(expr)) = (expr_stmt_node, yield_node, expr) else {
        return false;
    };

    if !tp.get_effect_context_flags(expr_stmt_node).intersects(EffectContextFlags::CanYieldEffect) {
        return false;
    }

    let Some(t) = tp.get_type_at_location(expr) else {
        return false;
    };
    let Some(effect) = tp.effect_yieldable_type(t) else {
        return false;
    };
    effect.a().flags().intersects(TypeFlags::Never)
}
