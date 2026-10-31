// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::checker::Type;
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::THIS_EFFECT_VALUE_IS_NEITHER_YIELDED_NOR_USED_IN_AN_ASSIGNMENT_EFFECT_FLOATING_EFFECT;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE_EFFECT_ABLE: &Message = &tsdiag::THIS_EFFECT_ABLE_0_VALUE_IS_NEITHER_YIELDED_NOR_ASSIGNED_TO_A_VARIABLE_EFFECT_FLOATING_EFFECT;

// go: internal/rules/floating_effect.go:16 floatingEffectResult 2d2146b073b9aacb
// floatingEffectResult holds information about a detected floating Effect expression.
pub(crate) struct FloatingEffectResult {
    // isStrict is true when the type's symbol name is exactly "Effect"
    pub(crate) is_strict: bool,
    // exprType is the checker type of the floating expression
    pub(crate) expr_type: P<Type>,
}

// go: internal/rules/floating_effect.go:25 FloatingEffect 10fa1f961ea67616
// FloatingEffect detects Effect values that are created as standalone
// expression statements and are neither yielded nor assigned.
pub static FLOATING_EFFECT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "floatingEffect",
    group: "correctness",
    description: "Detects Effect values that are neither yielded nor assigned",
    default_severity: Severity::Error,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code(), MESSAGE_EFFECT_ABLE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_floating_effect(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            if m.is_strict {
                diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
            } else {
                let type_name = ctx.checker().type_to_string_exported(m.expr_type);
                diags.push(ctx.new_diagnostic(
                    m.source_file,
                    m.location,
                    MESSAGE_EFFECT_ABLE,
                    &[],
                    &[&type_name],
                ));
            }
        }
        diags
    },
});

// go: internal/rules/floating_effect.go:52 FloatingEffectMatch 892e4e6ec68e9fc8
// FloatingEffectMatch holds the diagnostic location and expression needed by
// both the diagnostic rule and its quick-fix.
#[derive(Clone)]
pub struct FloatingEffectMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub expression: P<Node>,
    pub(crate) is_strict: bool,
    pub(crate) expr_type: P<Type>,
}

// go: internal/rules/floating_effect.go:61 AnalyzeFloatingEffect 97a7824d85703284
// AnalyzeFloatingEffect finds Effect values used as standalone expression statements.
//
// Rust: Go's checker argument is the parser's checker; its callee ignores it, so it is dropped.
pub fn analyze_floating_effect(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<FloatingEffectMatch> {
    let mut matches = Vec::new();
    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<FloatingEffectMatch>,
    ) -> bool {
        if let Some(result) = detect_floating_effect(tp, Some(node)) {
            let expression = node.as_expression_statement().expression();
            matches.push(FloatingEffectMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, expression),
                expression,
                is_strict: result.is_strict,
                expr_type: result.expr_type,
            });
        }

        node.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }
    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/floating_effect.go:91 detectFloatingEffect 44a80d935048c27f
// detectFloatingEffect checks if a node is an expression statement containing an Effect type
// that is neither yielded nor assigned. Returns nil if the node should not be reported,
// or a result with type info for selecting the appropriate diagnostic message.
//
// Rust: Go's checker argument is unused and dropped.
pub(crate) fn detect_floating_effect(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> Option<FloatingEffectResult> {
    // Must be an ExpressionStatement
    let node = node?;
    if node.kind() != Kind::ExpressionStatement {
        return None;
    }

    let expr_stmt = node.as_expression_statement();

    let expr = expr_stmt.expression();

    // Exclude assignment expressions
    if is_assignment_expression(Some(expr)) {
        return None;
    }

    let has_effect_type_id = tp.has_effect_type_id_at_location(expr);
    if !has_effect_type_id && tp.stream_type_at_location(expr).is_none() {
        return None;
    }

    // Cached by the AtLocation parsers above, which only succeed on a type.
    let t = tp.get_type_at_location(expr)?;

    if has_effect_type_id {
        // Full Effect validation.
        tp.effect_type(t)?;

        // Exclude Fiber types (considered valid floating operations)
        if tp.fiber_type(t).is_some() {
            return None;
        }

        // Exclude Effect subtypes (Exit, Option, Either, Pool, etc.)
        if tp.effect_subtype(t).is_some() {
            return None;
        }
    }

    // Determine if this is strictly an Effect or an Effect-able type
    let is_strict = tp.strict_effect_type(t).is_some();
    Some(FloatingEffectResult { is_strict, expr_type: t })
}

// go: internal/rules/floating_effect.go:143 isAssignmentExpression ad3ba8d34dd2ba22
// isAssignmentExpression checks if an expression is an assignment (=, ??=, &&=, ||=).
pub(crate) fn is_assignment_expression(node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if node.kind() != Kind::BinaryExpression {
        return false;
    }

    let bin_expr = node.as_binary_expression();

    matches!(
        bin_expr.operator_token().kind(),
        Kind::EqualsToken
            | Kind::QuestionQuestionEqualsToken
            | Kind::AmpersandAmpersandEqualsToken
            | Kind::BarBarEqualsToken
    )
}
