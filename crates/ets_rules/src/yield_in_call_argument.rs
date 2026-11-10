use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::scanner;
use ets_typeparser::{EffectContextFlags, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_YIELD_ASTERISK_IS_USED_AS_A_CALL_ARGUMENT_YIELD_IT_INTO_A_CONST_FIRST_OR_MAP_THE_EFFECT_SO_RUNNING_THE_EFFECT_AND_BUILDING_THE_CALL_STAY_SEPARATE_STEPS_EFFECT_YIELD_IN_CALL_ARGUMENT;

// go: internal/rules/yield_in_call_argument.go:18 YieldInCallArgument c8edbc63e20bf7be
// YieldInCallArgument suggests yielding into a const before a call instead of
// passing a yield* directly as a call or new argument in Effect generators, so
// running the effect and building the call stay separate steps.
pub static YIELD_IN_CALL_ARGUMENT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "yieldInCallArgument",
    group: "style",
    description: "Suggests yielding into a const before a call instead of passing a yield* directly as a call or new argument in Effect generators",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_yield_in_call_argument(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/yield_in_call_argument.go:42 YieldInCallArgumentMatch e9087995de464a95
#[derive(Clone, Copy)]
pub struct YieldInCallArgumentMatch {
    pub location: TextRange,
    // Yield is the yield* expression passed as an argument.
    pub yield_: P<Node>,
    // Argument is Yield with its enclosing parentheses, the node the fix replaces.
    pub argument: P<Node>,
}

// go: internal/rules/yield_in_call_argument.go:50 AnalyzeYieldInCallArgument 0dd31bdd2a66fc04
pub fn analyze_yield_in_call_argument(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<YieldInCallArgumentMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function; Go's `node == nil` check drops (children are
    // never nil).
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<YieldInCallArgumentMatch>,
    ) -> bool {
        if node.kind() == Kind::YieldExpression {
            let yield_expr = node.as_yield_expression();
            if yield_expr.asterisk_token().is_some() && yield_expr.expression().is_some() {
                if let Some(arg) = yield_in_call_argument_argument(node)
                    && tp
                        .get_effect_context_flags(node)
                        .intersects(EffectContextFlags::CanYieldEffect)
                {
                    matches.push(YieldInCallArgumentMatch {
                        location: scanner::get_error_range_for_node(sf, node),
                        yield_: node,
                        argument: arg,
                    });
                }
            }
        }

        // ets:ast-walk finds yield* call arguments; new expressions and multi-argument calls to a non-dual callee produce no flow step
        node.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/yield_in_call_argument.go:83 yieldInCallArgumentArgument fec4d96a7bdb2e37
// yieldInCallArgumentArgument returns the argument of a call or new expression
// that is node, or node in parentheses, or nil when node is not an argument.
pub(crate) fn yield_in_call_argument_argument(node: P<Node>) -> Option<P<Node>> {
    let call = ast::walk_up_parenthesized_expressions(node.parent())?;
    if !ast::is_call_or_new_expression(call) {
        return None;
    }
    call.arguments().iter().copied().find(|&argument| ast::skip_parentheses(argument) == node)
}
