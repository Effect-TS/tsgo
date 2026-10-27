use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_VOID_REPRESENTS_THE_SAME_OUTCOME_AS_EFFECT_SUCCEED_UNDEFINED_OR_EFFECT_SUCCEED_VOID_0_EFFECT_EFFECT_SUCCEED_WITH_VOID;

// go: internal/rules/effect_succeed_with_void.go:15 EffectSucceedWithVoid 9b81fc628ebdf43f
// EffectSucceedWithVoid suggests using Effect.void instead of Effect.succeed(undefined) or Effect.succeed(void 0).
pub static EFFECT_SUCCEED_WITH_VOID: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectSucceedWithVoid",
    group: "style",
    description: "Suggests using Effect.void instead of Effect.succeed(undefined) or Effect.succeed(void 0)",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_effect_succeed_with_void(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/effect_succeed_with_void.go:34 EffectSucceedWithVoidMatch 8f505c3fa2531b86
// EffectSucceedWithVoidMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the effectSucceedWithVoid pattern.
#[derive(Clone)]
pub struct EffectSucceedWithVoidMatch {
    pub source_file: P<SourceFile>, // The source file where the diagnostic should be reported
    pub location: TextRange,        // The pre-computed error range for this match
    pub call_node: P<Node>,         // The Effect.succeed(...) call expression (replacement target)
}

// go: internal/rules/effect_succeed_with_void.go:41 isVoidExpression 5e71e29c88500c9e
// isVoidExpression checks if a node is `undefined`, `void 0`, or a parenthesized version of either.
//
// Rust: Go dereferences the node without a nil check; a nil node is `None` and is not a void expression.
pub(crate) fn is_void_expression(node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    // Unwrap parenthesized expressions
    let node = ast::skip_parentheses(node);

    // Check for `undefined`
    if node.kind() == Kind::Identifier && ets_shim::scanner::get_text_of_node(node) == "undefined" {
        return true;
    }

    // Check for `void 0`
    if node.kind() == Kind::VoidExpression {
        let operand = node.as_void_expression().expression();
        if operand.kind() == Kind::NumericLiteral
            && ets_shim::scanner::get_text_of_node(operand) == "0"
        {
            return true;
        }
    }

    false
}

// go: internal/rules/effect_succeed_with_void.go:63 AnalyzeEffectSucceedWithVoid aaad7c7401ebf93f
// AnalyzeEffectSucceedWithVoid finds all Effect.succeed(undefined) and Effect.succeed(void 0) calls
// that can be replaced with Effect.void.
//
// Rust: Go's checker argument is unused and dropped.
pub fn analyze_effect_succeed_with_void(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<EffectSucceedWithVoidMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<EffectSucceedWithVoidMatch>,
    ) -> bool {
        if n.kind() == Kind::CallExpression {
            let call = n.as_call_expression();
            let arguments = call.arguments().nodes();
            if !arguments.is_empty() {
                let arg = arguments[0];
                if is_void_expression(Some(arg))
                    && tp.is_node_reference_to_effect_module_api(call.expression(), "succeed")
                {
                    matches.push(EffectSucceedWithVoidMatch {
                        source_file: sf,
                        location: ets_shim::scanner::get_error_range_for_node(sf, n),
                        call_node: n,
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
