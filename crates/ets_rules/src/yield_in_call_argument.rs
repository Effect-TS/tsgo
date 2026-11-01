use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, FindAncestorResult, Kind, Node, NodeFlags, SourceFile};
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

// go: internal/rules/yield_in_call_argument.go:42 YieldInCallArgumentMatch d71d64902a7d5fc4
#[derive(Clone, Copy)]
pub struct YieldInCallArgumentMatch {
    pub location: TextRange,
    // Yield is the yield* expression passed as an argument.
    pub yield_: P<Node>,
    // Argument is Yield with its enclosing parentheses, the node the fix replaces.
    pub argument: P<Node>,
    // Statement is where the fix inserts the hoisted const; nil when hoisting
    // could change evaluation order, and the match is reported without a fix.
    pub statement: Option<P<Node>>,
}

// go: internal/rules/yield_in_call_argument.go:53 AnalyzeYieldInCallArgument 638a8b092e03d0f3
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
                        statement: yield_in_call_argument_hoist_anchor(arg),
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

// go: internal/rules/yield_in_call_argument.go:87 yieldInCallArgumentArgument fec4d96a7bdb2e37
// yieldInCallArgumentArgument returns the argument of a call or new expression
// that is node, or node in parentheses, or nil when node is not an argument.
pub(crate) fn yield_in_call_argument_argument(node: P<Node>) -> Option<P<Node>> {
    let call = ast::walk_up_parenthesized_expressions(node.parent())?;
    if !ast::is_call_or_new_expression(call) {
        return None;
    }
    call.arguments().iter().copied().find(|&argument| ast::skip_parentheses(argument) == node)
}

// go: internal/rules/yield_in_call_argument.go:103 yieldInCallArgumentHoistAnchor df8ab9f25c95dfc9
// yieldInCallArgumentHoistAnchor returns the statement before which the yield*
// argument can be hoisted without changing what runs before the effect, or nil.
// It climbs from the argument through an allowlist of parent kinds; everything
// evaluated before the argument along the way must be side-effect free.
pub(crate) fn yield_in_call_argument_hoist_anchor(arg: P<Node>) -> Option<P<Node>> {
    let mut child = arg;
    let mut anchor = ast::find_ancestor_or_quit(arg.parent(), |p| {
        let result = yield_in_call_argument_hoist_step(p, child);
        child = p;
        result
    })?;
    if anchor.kind() == Kind::VariableDeclaration {
        anchor = anchor
            .parent()
            .and_then(|list| list.parent())
            .expect("the hoist step checked the variable statement");
    }
    yield_in_call_argument_finalize_anchor(anchor)
}

// go: internal/rules/yield_in_call_argument.go:123 yieldInCallArgumentHoistStep 72c201a66a81b6b2
// yieldInCallArgumentHoistStep decides one step of the climb from child to its
// parent p: climb on, stop at p as the statement (or the single const
// declaration) to hoist before, or quit because hoisting could change what runs
// before the effect.
pub(crate) fn yield_in_call_argument_hoist_step(p: P<Node>, child: P<Node>) -> FindAncestorResult {
    match p.kind() {
        Kind::ParenthesizedExpression
        | Kind::AsExpression
        | Kind::NonNullExpression
        | Kind::YieldExpression => FindAncestorResult::False,
        Kind::CallExpression | Kind::NewExpression => {
            let expression = p.expression().expect("a call or new expression has a callee");
            if child == expression {
                return FindAncestorResult::False;
            }
            if p.flags().intersects(NodeFlags::OptionalChain)
                || !yield_in_call_argument_is_side_effect_free(expression)
            {
                return FindAncestorResult::Quit;
            }
            for &argument in p.arguments() {
                if argument == child {
                    break;
                }
                if !yield_in_call_argument_is_side_effect_free(argument) {
                    return FindAncestorResult::Quit;
                }
            }
            FindAncestorResult::False
        }
        Kind::PropertyAccessExpression => {
            if Some(child) != p.expression() {
                return FindAncestorResult::Quit;
            }
            FindAncestorResult::False
        }
        Kind::ElementAccessExpression => {
            let expression = p.expression().expect("an element access has an expression");
            if child != expression
                && (p.flags().intersects(NodeFlags::OptionalChain)
                    || !yield_in_call_argument_is_side_effect_free(expression))
            {
                return FindAncestorResult::Quit;
            }
            FindAncestorResult::False
        }
        Kind::BinaryExpression => {
            let binary = p.as_binary_expression();
            if child == binary.left() {
                return FindAncestorResult::False;
            }
            let operator = binary.operator_token().kind();
            if operator == Kind::AmpersandAmpersandToken
                || operator == Kind::BarBarToken
                || operator == Kind::QuestionQuestionToken
                || ast::is_logical_or_coalescing_assignment_operator(operator)
                || !yield_in_call_argument_is_side_effect_free(binary.left())
            {
                return FindAncestorResult::Quit;
            }
            FindAncestorResult::False
        }
        Kind::ConditionalExpression => {
            if child != p.as_conditional_expression().condition() {
                return FindAncestorResult::Quit;
            }
            FindAncestorResult::False
        }
        Kind::ExpressionStatement | Kind::ReturnStatement => FindAncestorResult::True,
        Kind::IfStatement => {
            if child != p.as_if_statement().expression() {
                return FindAncestorResult::Quit;
            }
            FindAncestorResult::True
        }
        Kind::VariableDeclaration => {
            let list = p.parent();
            if Some(child) != p.initializer()
                || list.is_none_or(|list| {
                    list.kind() != Kind::VariableDeclarationList
                        || !list.flags().intersects(NodeFlags::Const)
                        || list.as_variable_declaration_list().declarations().nodes().len() != 1
                        || list
                            .parent()
                            .is_none_or(|parent| parent.kind() != Kind::VariableStatement)
                })
            {
                return FindAncestorResult::Quit;
            }
            FindAncestorResult::True
        }
        _ => FindAncestorResult::Quit,
    }
}

// go: internal/rules/yield_in_call_argument.go:190 yieldInCallArgumentFinalizeAnchor 6655bd2c7ae49357
// yieldInCallArgumentFinalizeAnchor accepts a statement only when it sits in a
// statement list, so a const can be inserted right before it.
pub(crate) fn yield_in_call_argument_finalize_anchor(statement: P<Node>) -> Option<P<Node>> {
    let parent = statement.parent()?;
    match parent.kind() {
        Kind::Block | Kind::CaseClause | Kind::DefaultClause => Some(statement),
        _ => None,
    }
}

// go: internal/rules/yield_in_call_argument.go:203 yieldInCallArgumentIsSideEffectFree cdcd23d12fe7c18b
// yieldInCallArgumentIsSideEffectFree reports whether evaluating node cannot run
// code: identifiers, literals and non-optional property accesses on them.
pub(crate) fn yield_in_call_argument_is_side_effect_free(node: P<Node>) -> bool {
    let node = ast::skip_parentheses(node);
    match node.kind() {
        Kind::Identifier
        | Kind::StringLiteral
        | Kind::NumericLiteral
        | Kind::BigIntLiteral
        | Kind::NoSubstitutionTemplateLiteral
        | Kind::RegularExpressionLiteral
        | Kind::TrueKeyword
        | Kind::FalseKeyword
        | Kind::NullKeyword => true,
        Kind::PrefixUnaryExpression => {
            let prefix = node.as_prefix_unary_expression();
            prefix.operator() == Kind::MinusToken && prefix.operand().kind() == Kind::NumericLiteral
        }
        Kind::PropertyAccessExpression => {
            !node.flags().intersects(NodeFlags::OptionalChain)
                && yield_in_call_argument_is_side_effect_free(
                    node.expression().expect("a property access has an expression"),
                )
        }
        _ => false,
    }
}
