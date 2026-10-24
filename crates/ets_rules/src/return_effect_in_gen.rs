use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectContextFlags, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::This_generator_returns_an_Effect_able_value_directly_which_produces_a_nested_Effect_Effect_If_the_intended_result_is_the_inner_Effect_value_return_yield_Asterisk_represents_that_form_effect_returnEffectInGen;

// go: internal/rules/return_effect_in_gen.go:16 ReturnEffectInGen
// ReturnEffectInGen detects return statements inside Effect generators
// that return an Effect-able type, which would result in nested Effect<Effect<...>>.
pub static RETURN_EFFECT_IN_GEN: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "returnEffectInGen",
    group: "antipattern",
    description: "Warns when returning an Effect in a generator causes nested Effect<Effect<...>>",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_return_effect_in_gen(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/return_effect_in_gen.go:35 ReturnEffectInGenMatch
// ReturnEffectInGenMatch holds the diagnostic and the return statement node needed
// by both the diagnostic rule and the quick-fix.
#[derive(Clone)]
pub struct ReturnEffectInGenMatch {
    pub source_file: P<SourceFile>, // The source file where this match was found
    pub location: TextRange,        // The pre-computed error range for this match
    pub return_node: P<Node>,       // The return statement AST node
}

// go: internal/rules/return_effect_in_gen.go:43 AnalyzeReturnEffectInGen
// AnalyzeReturnEffectInGen finds all return statements inside Effect generators
// that return an Effect-able type, returning matches with both the diagnostic and the return node.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); its callee ignores it, so it is dropped.
pub fn analyze_return_effect_in_gen(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<ReturnEffectInGenMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<ReturnEffectInGenMatch>,
    ) -> bool {
        if n.kind() == Kind::ReturnStatement && check_return_effect_in_gen_scope(tp, n) {
            matches.push(ReturnEffectInGenMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, n),
                return_node: n,
            });
        }

        n.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/return_effect_in_gen.go:72 checkReturnEffectInGenScope
// checkReturnEffectInGenScope checks if a return statement inside an Effect generator
// is returning an Effect-able type (which would cause nested Effect<Effect<...>>).
//
// Rust: Go's unused checker and source file parameters are dropped.
pub(crate) fn check_return_effect_in_gen_scope(tp: &mut TypeParser<'_>, n: P<Node>) -> bool {
    let return_stmt = n.as_return_statement();
    let Some(expression) = return_stmt.expression() else {
        return false;
    };

    // return yield* ... is the correct pattern, skip it
    if expression.kind() == Kind::YieldExpression {
        return false;
    }

    if !tp.get_effect_context_flags(n).intersects(EffectContextFlags::CanYieldEffect) {
        return false;
    }

    if tp.node_cannot_be_effect_library_type(expression) {
        return false;
    }

    let Some(t) = tp.get_type_at_location(expression) else {
        return false;
    };

    if !tp.strict_is_effect_type(t) {
        return false;
    }

    true
}
