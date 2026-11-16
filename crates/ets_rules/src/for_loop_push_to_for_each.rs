use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectContextFlags, PipingFlow, PipingFlowSubject, PipingFlowTransformation,
    TransformationKind, TypeParser,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_FOR_EACH_EXPRESSES_THIS_FOR_OF_LOOP_THAT_PUSHES_YIELDED_RESULTS_INTO_AN_ACCUMULATOR_MORE_DIRECTLY_AND_MAKES_ORDERING_AND_CONCURRENCY_EXPLICIT_EFFECT_FOR_LOOP_PUSH_TO_FOR_EACH;

// go: internal/rules/for_loop_push_to_for_each.go:16 ForLoopPushToForEach a0555f29543a9fbf
// ForLoopPushToForEach suggests Effect.forEach instead of a for-of loop inside
// an Effect generator that only pushes yielded Effect results into an empty
// accumulator declared just before the loop.
pub static FOR_LOOP_PUSH_TO_FOR_EACH: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "forLoopPushToForEach",
    group: "style",
    description: "Suggests Effect.forEach instead of a for-of loop that pushes yielded Effect results into an accumulator",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_for_loop_push_to_for_each_exported(ctx.type_parser, source_file);
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/for_loop_push_to_for_each.go:41 ForLoopPushToForEachMatch eb67b293b11cf8ec
// ForLoopPushToForEachMatch holds the nodes needed by the diagnostic and its fix.
#[derive(Clone)]
pub struct ForLoopPushToForEachMatch {
    pub location: TextRange,
    // Loop is the for-of statement; its initializer holds the loop binding and
    // its expression the iterable.
    pub loop_: P<Node>,
    // Declaration is the accumulator's `const acc = []` variable statement.
    pub declaration: P<Node>,
    // Flow is `yield* f(x)` piped into `acc.push`.
    pub flow: PipingFlow,
}

// go: internal/rules/for_loop_push_to_for_each.go:59 AnalyzeForLoopPushToForEach b0422cbc7a180b22
// AnalyzeForLoopPushToForEach finds `for (const x of xs) acc.push(yield* f(x))`
// loops inside Effect generators where acc is `const acc = []`, declared right
// before the loop or separated from it only by other declarations.
//
// The push call is matched as a piping flow, `yield* f(x)` piped into
// `acc.push`; the loop and the accumulator declaration around it are
// statements, which flows do not represent, and are matched on the syntax tree.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), and Go's nil parser and checker checks
// have no counterpart (both are references). Go's `callee == nil` check drops: a transformation's callee is
// non-optional here.
pub fn analyze_for_loop_push_to_for_each_exported(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<ForLoopPushToForEachMatch> {
    let is_yield_star = |_: &mut TypeParser<'_>, subject: &PipingFlowSubject| -> bool {
        let Some(node) = subject.node() else {
            return false;
        };
        if node.kind() != Kind::YieldExpression {
            return false;
        }
        let yield_ = node.as_yield_expression();
        yield_.asterisk_token().is_some() && yield_.expression().is_some()
    };
    // A plain call keeps push bound to the accumulator; pipe(x, acc.push) would not.
    let is_push = |tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| -> bool {
        let callee = transformation.callee();
        if transformation.kind() != TransformationKind::Call
            || transformation.args().len() != 0
            || callee.kind() != Kind::PropertyAccessExpression
        {
            return false;
        }
        let access = callee.as_property_access_expression();
        access.name().text() == "push"
            && access.expression().kind() == Kind::Identifier
            && tp.is_native_array_method_reference(callee, "push")
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, false);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        if !PipingFlow::matches_exactly(Some(flow), tp, &is_yield_star, &[&is_push]) {
            continue;
        }
        if let Some(m) = analyze_for_loop_push_to_for_each(tp, sf, flow) {
            matches.push(m);
        }
    }
    matches
}

// go: internal/rules/for_loop_push_to_for_each.go:96 analyzeForLoopPushToForEach f39e3155bb624562
// Rust: the exported `AnalyzeForLoopPushToForEach` is `analyze_for_loop_push_to_for_each_exported` (the naming
// rule for a Go package with both). The for-of initializer and the declaration list are never nil in tsrs
// (non-optional fields), so Go's nil checks drop.
pub(crate) fn analyze_for_loop_push_to_for_each(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    flow: &PipingFlow,
) -> Option<ForLoopPushToForEachMatch> {
    let loop_ = for_loop_push_to_for_each_loop(flow.node())?;
    let for_of = loop_.as_for_in_or_of_statement();
    if for_of.await_modifier().is_some()
        || for_of.initializer().kind() != Kind::VariableDeclarationList
        || !ast::is_var_const(for_of.initializer())
    {
        return None;
    }
    let loop_declarations =
        for_of.initializer().as_variable_declaration_list().declarations().nodes();
    if loop_declarations.len() != 1 || loop_declarations[0].initializer().is_some() {
        return None;
    }
    let binding = loop_declarations[0].as_variable_declaration().name();

    let receiver =
        flow.transformations().at(0).callee().as_property_access_expression().expression();
    let yielded = flow
        .subject()
        .node()
        .expect("checked by isYieldStar")
        .as_yield_expression()
        .expression()
        .expect("checked by isYieldStar");

    if loop_.parent().is_none_or(|parent| parent.kind() != Kind::Block) {
        return None;
    }
    let (declaration, between) = for_loop_push_to_for_each_accumulator(loop_, receiver.text())?;
    let accumulator = declaration
        .as_variable_statement()
        .declaration_list()
        .as_variable_declaration_list()
        .declarations()
        .nodes()[0]
        .as_variable_declaration();

    if tp.get_effect_context_flags(loop_) & EffectContextFlags::CanYieldEffect
        == EffectContextFlags::None
    {
        return None;
    }

    let accumulator_symbol = tp.get_symbol_at_location(accumulator.name())?;
    if tp.get_symbol_at_location(receiver) != Some(accumulator_symbol) {
        return None;
    }
    for node in [for_of.expression(), binding, yielded].into_iter().chain(between) {
        if !tp.references_to_symbol(node, accumulator_symbol).is_empty() {
            return None;
        }
    }

    tp.effect_type_at_location(yielded)?;

    Some(ForLoopPushToForEachMatch {
        location: ets_shim::scanner::get_error_range_for_node(sf, loop_),
        loop_,
        declaration,
        flow: flow.clone(),
    })
}

// go: internal/rules/for_loop_push_to_for_each.go:151 forLoopPushToForEachLoop aa82aa0bf0890b92
// forLoopPushToForEachLoop returns the for-of statement whose body is only the
// push call, either as the statement itself or as the only statement of a block.
pub(crate) fn for_loop_push_to_for_each_loop(push: P<Node>) -> Option<P<Node>> {
    if ast::is_optional_chain(push) {
        return None;
    }
    let statement = push.parent()?;
    if statement.kind() != Kind::ExpressionStatement {
        return None;
    }
    let mut body = statement;
    if let Some(parent) = statement.parent()
        && parent.kind() == Kind::Block
    {
        if parent.as_block().statements().nodes().len() != 1 {
            return None;
        }
        body = parent;
    }
    let loop_ = body.parent()?;
    if loop_.kind() != Kind::ForOfStatement || loop_.as_for_in_or_of_statement().statement() != body
    {
        return None;
    }
    Some(loop_)
}

// go: internal/rules/for_loop_push_to_for_each.go:176 forLoopPushToForEachAccumulator 7ac5819009be9613
// forLoopPushToForEachAccumulator walks back from loop over variable statements
// only, and returns the `const name = []` statement declaring the accumulator
// together with the declarations passed on the way.
//
// Rust: Go's `(nil, nil)` is `None`.
pub(crate) fn for_loop_push_to_for_each_accumulator(
    loop_: P<Node>,
    name: &str,
) -> Option<(P<Node>, Vec<P<Node>>)> {
    let statements = loop_.parent()?.as_block().statements().nodes();
    // Rust: Go's index -1 (loop not found) runs no iteration below.
    let index = statements.iter().position(|&statement| statement == loop_)?;
    let mut between = Vec::new();
    for i in (0..index).rev() {
        let statement = statements[i];
        if statement.kind() != Kind::VariableStatement {
            return None;
        }
        let list = statement.as_variable_statement().declaration_list();
        let declarations = list.as_variable_declaration_list().declarations().nodes();
        let mut declares_name = false;
        for declaration in declarations {
            let declaration_name = declaration.as_variable_declaration().name();
            if declaration_name.kind() == Kind::Identifier && declaration_name.text() == name {
                declares_name = true;
                break;
            }
        }
        if !declares_name {
            between.push(statement);
            continue;
        }
        if declarations.len() != 1 || !ast::is_var_const(list) {
            return None;
        }
        let initializer = declarations[0].initializer()?;
        if initializer.kind() != Kind::ArrayLiteralExpression
            || !initializer.as_array_literal_expression().elements().nodes().is_empty()
        {
            return None;
        }
        return Some((statement, between));
    }
    None
}
