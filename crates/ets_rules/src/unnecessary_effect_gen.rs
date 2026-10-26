use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::checker::TypeFlags;
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::TypeParser;

// go: internal/rules/unnecessary_effect_gen.go:15 UnnecessaryEffectGen
// UnnecessaryEffectGen suggests removing Effect.gen when it contains only a single return statement.
pub static UNNECESSARY_EFFECT_GEN: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "unnecessaryEffectGen",
    group: "style",
    description: "Suggests removing Effect.gen when it contains only a single return statement",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        tsdiag::THIS_EFFECT_GEN_CONTAINS_A_SINGLE_RETURN_STATEMENT_EFFECT_UNNECESSARY_EFFECT_GEN
            .code(),
    ],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_unnecessary_effect_gen(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                &tsdiag::THIS_EFFECT_GEN_CONTAINS_A_SINGLE_RETURN_STATEMENT_EFFECT_UNNECESSARY_EFFECT_GEN,
                &[],
                &[],
            ));
        }
        diags
    },
});

// go: internal/rules/unnecessary_effect_gen.go:34 UnnecessaryEffectGenMatch
// UnnecessaryEffectGenMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the unnecessaryEffectGen pattern.
#[derive(Clone)]
pub struct UnnecessaryEffectGenMatch {
    pub source_file: P<SourceFile>, // The source file where this match was found
    pub location: TextRange,        // The pre-computed error range for this match
    pub call_node: P<Node>,         // The Effect.gen(...) call expression (replacement target)
    pub yielded_expression: P<Node>, // The expression being yield*-ed (the replacement value)
    pub explicit_return: bool,      // Whether the statement had an explicit return
    pub success_is_void: bool,      // Whether the yielded Effect's success type is void-like
    pub effect_module_node: Option<P<Node>>, // The Effect module identifier (for Effect.asVoid wrapping)
}

// go: internal/rules/unnecessary_effect_gen.go:46 AnalyzeUnnecessaryEffectGen
// AnalyzeUnnecessaryEffectGen finds all Effect.gen calls that contain only a single
// yield* statement and can be replaced with the yielded expression directly.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); Go ignores it, so it is dropped.
pub fn analyze_unnecessary_effect_gen(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<UnnecessaryEffectGenMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<UnnecessaryEffectGenMatch>,
    ) -> bool {
        if n.kind() == Kind::CallExpression
            && let Some(m) = analyze_unnecessary_effect_gen_node(tp, sf, n)
        {
            matches.push(m);
        }

        n.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/unnecessary_effect_gen.go:69 analyzeUnnecessaryEffectGenNode
pub(crate) fn analyze_unnecessary_effect_gen_node(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    n: P<Node>,
) -> Option<UnnecessaryEffectGenMatch> {
    let gen_result = tp.effect_gen_call(n)?;

    // Must have exactly one argument (just the generator, no options object)
    if gen_result.call().arguments().len() != 1 {
        return None;
    }

    let body = gen_result.body()?;
    if body.kind() != Kind::Block {
        return None;
    }

    let statements = body.statements();
    if statements.len() != 1 {
        return None;
    }

    let stmt = statements[0];

    let (expr, explicit_return) = match stmt.kind() {
        Kind::ReturnStatement => (stmt.as_return_statement().expression()?, true),
        Kind::ExpressionStatement => (stmt.as_expression_statement().expression(), false),
        _ => return None,
    };

    // Must be a yield* expression
    if expr.kind() != Kind::YieldExpression {
        return None;
    }
    let yield_ = expr.as_yield_expression();
    yield_.asterisk_token()?;
    let yielded_expr = yield_.expression()?;

    if ets_shim::checker::for_each_yield_expression(yielded_expr, is_yield_star_expression) {
        return None;
    }

    // Determine if the success type is void-like
    let mut success_is_void = false;
    let mut t = None;
    if !tp.node_cannot_be_effect_library_type(yielded_expr) {
        t = tp.get_type_at_location(yielded_expr);
    }
    if let Some(t) = t
        && let Some(effect) = tp.effect_type(t)
    {
        success_is_void = effect.a().flags().intersects(TypeFlags::VoidLike);
    }

    Some(UnnecessaryEffectGenMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, n),
        call_node: n,
        yielded_expression: yielded_expr,
        explicit_return,
        success_is_void,
        effect_module_node: gen_result.effect_module(),
    })
}

// go: internal/rules/unnecessary_effect_gen.go:147 isYieldStarExpression
pub(crate) fn is_yield_star_expression(expr: P<Node>) -> bool {
    expr.as_yield_expression().asterisk_token().is_some()
}
