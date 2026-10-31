use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile, Symbol};
use ets_shim::checker::SignatureKind;
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectContextFlags, PipingFlow, PipingFlowSubject, PipingFlowTransformation,
    TransformationKind, TypeParser,
};
use rustc_hash::FxHashSet;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_FOR_EACH_EXPRESSES_THIS_FOR_OF_LOOP_THAT_PUSHES_YIELDED_RESULTS_INTO_AN_ACCUMULATOR_MORE_DIRECTLY_AND_MAKES_ORDERING_AND_CONCURRENCY_EXPLICIT_EFFECT_FOR_LOOP_PUSH_TO_FOR_EACH;

// go: internal/rules/for_loop_push_to_for_each.go:17 ForLoopPushToForEach 1748cdb77fb82ef0
// ForLoopPushToForEach suggests Effect.forEach instead of a for-of loop inside
// an Effect generator that only pushes yielded Effect results into an empty
// accumulator declared just before the loop.
pub static FOR_LOOP_PUSH_TO_FOR_EACH: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "forLoopPushToForEach",
    group: "style",
    description: "Suggests Effect.forEach instead of a for-of loop that pushes yielded Effect results into an accumulator",
    default_severity: Severity::Suggestion,
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

// go: internal/rules/for_loop_push_to_for_each.go:42 ForLoopPushToForEachMatch 7ab8a369126a4aab
// ForLoopPushToForEachMatch holds the nodes needed by the diagnostic and its fix.
#[derive(Clone)]
pub struct ForLoopPushToForEachMatch {
    pub location: TextRange,
    pub loop_: P<Node>,
    // Declaration is the accumulator's variable statement.
    pub declaration: P<Node>,
    pub accumulator_name: P<Node>,
    // AccumulatorType is the accumulator's type annotation, or nil.
    pub accumulator_type: Option<P<Node>>,
    pub iterable: P<Node>,
    // Binding is the loop variable's name, possibly a binding pattern.
    pub binding: P<Node>,
    pub yielded: P<Node>,
    // PointFreeCallee is the callee Effect.forEach can receive directly, or nil
    // when the fix has to keep an arrow around Yielded.
    pub point_free_callee: Option<P<Node>>,
    // CanFix reports whether replacing the accumulator with the result of
    // Effect.forEach is safe: the accumulator is not mutated after the loop and
    // Binding and Yielded, which the fix moves into an arrow function, contain
    // no yield of the generator.
    pub can_fix: bool,
}

// go: internal/rules/for_loop_push_to_for_each.go:71 AnalyzeForLoopPushToForEach 3ed51162179f036a
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

// go: internal/rules/for_loop_push_to_for_each.go:108 analyzeForLoopPushToForEach c9540ab93b8820ab
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
    let name = receiver.text();
    if references_symbol(tp, Some(for_of.expression()), name, accumulator_symbol)
        || references_symbol(tp, Some(binding), name, accumulator_symbol)
        || references_symbol(tp, Some(yielded), name, accumulator_symbol)
    {
        return None;
    }
    for statement in between {
        if references_symbol(tp, Some(statement), name, accumulator_symbol) {
            return None;
        }
    }

    tp.effect_type_at_location(yielded)?;

    Some(ForLoopPushToForEachMatch {
        location: ets_shim::scanner::get_error_range_for_node(sf, loop_),
        loop_,
        declaration,
        accumulator_name: accumulator.name(),
        accumulator_type: accumulator.type_(),
        iterable: for_of.expression(),
        binding,
        yielded,
        point_free_callee: for_loop_push_to_for_each_point_free_callee(tp, binding, yielded),
        can_fix: !mutated_after_loop(tp, loop_, name, accumulator_symbol)
            && !contains_yield(binding)
            && !contains_yield(yielded),
    })
}

// go: internal/rules/for_loop_push_to_for_each.go:175 forLoopPushToForEachLoop aa82aa0bf0890b92
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

// go: internal/rules/for_loop_push_to_for_each.go:200 forLoopPushToForEachAccumulator 7ac5819009be9613
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

// go: internal/rules/for_loop_push_to_for_each.go:244 referencesSymbol 30ca1ac82b9c069f
// referencesSymbol reports whether node contains an identifier, or a shorthand
// property, that resolves to symbol. Identifiers are compared by name first so
// most nodes skip the symbol lookup.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn references_symbol(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
    name: &str,
    symbol: P<Symbol>,
) -> bool {
    // Rust: Go's recursive `visit` closure is a nested function.
    fn visit(tp: &mut TypeParser<'_>, n: P<Node>, name: &str, symbol: P<Symbol>) -> bool {
        match n.kind() {
            Kind::ShorthandPropertyAssignment => {
                if n.name().is_some_and(|n| n.text() == name)
                    && tp.checker().get_shorthand_assignment_value_symbol(Some(n)) == Some(symbol)
                {
                    return true;
                }
            }
            Kind::Identifier => {
                return n.text() == name && tp.get_symbol_at_location(n) == Some(symbol);
            }
            _ => {}
        }
        n.for_each_child(&mut |child| visit(tp, child, name, symbol))
    }
    node.is_some_and(|node| visit(tp, node, name, symbol))
}

// go: internal/rules/for_loop_push_to_for_each.go:263 forLoopPushToForEachMutatingMethods 98a183b4d25fcfac
// forLoopPushToForEachMutatingMethods are the Array methods that change the
// array they are called on.
pub(crate) static FOR_LOOP_PUSH_TO_FOR_EACH_MUTATING_METHODS: LazyLock<FxHashSet<&'static str>> =
    LazyLock::new(|| {
        ["push", "pop", "shift", "unshift", "splice", "sort", "reverse", "fill", "copyWithin"]
            .into_iter()
            .collect()
    });

// go: internal/rules/for_loop_push_to_for_each.go:271 mutatedAfterLoop 683af25c1072e348
// mutatedAfterLoop reports whether a statement after loop in the same block
// calls a mutating Array method on the accumulator, writes one of its
// elements or its length, or deletes one of its elements.
pub(crate) fn mutated_after_loop(
    tp: &mut TypeParser<'_>,
    loop_: P<Node>,
    name: &str,
    symbol: P<Symbol>,
) -> bool {
    // Rust: Go's `isMutation` closure is a nested function.
    fn is_mutation(reference: P<Node>) -> bool {
        let Some(parent) = reference.parent() else {
            return false;
        };
        match parent.kind() {
            Kind::PropertyAccessExpression => {
                let access = parent.as_property_access_expression();
                if access.expression() != reference {
                    return false;
                }
                let member = access.name().text();
                FOR_LOOP_PUSH_TO_FOR_EACH_MUTATING_METHODS.contains(member)
                    || member == "length" && ast::is_write_access(parent)
            }
            Kind::ElementAccessExpression => {
                if parent.as_element_access_expression().expression() != reference {
                    return false;
                }
                ast::is_write_access(parent)
                    || parent.parent().is_some_and(|p| p.kind() == Kind::DeleteExpression)
            }
            _ => false,
        }
    }
    // Rust: Go's recursive `visit` closure is a nested function.
    fn visit(tp: &mut TypeParser<'_>, n: P<Node>, name: &str, symbol: P<Symbol>) -> bool {
        if n.kind() == Kind::Identifier {
            return n.text() == name
                && tp.get_symbol_at_location(n) == Some(symbol)
                && is_mutation(n);
        }
        n.for_each_child(&mut |child| visit(tp, child, name, symbol))
    }
    let mut after = false;
    let parent = loop_.parent().expect("the loop is a statement of a block");
    for &statement in parent.as_block().statements().nodes() {
        if statement == loop_ {
            after = true;
            continue;
        }
        if after && visit(tp, statement, name, symbol) {
            return true;
        }
    }
    false
}

// go: internal/rules/for_loop_push_to_for_each.go:316 containsYield 12ed8081aea54472
// containsYield reports whether node contains a yield expression of the
// enclosing generator, which could not move into an arrow function.
pub(crate) fn contains_yield(node: P<Node>) -> bool {
    // Rust: Go's recursive `visit` closure is a nested function.
    fn visit(n: P<Node>) -> bool {
        if n.kind() == Kind::YieldExpression {
            return true;
        }
        if ast::is_function_like(n) {
            return false;
        }
        n.for_each_child(&mut |child| visit(child))
    }
    visit(node)
}

// go: internal/rules/for_loop_push_to_for_each.go:336 forLoopPushToForEachPointFreeCallee bd7a49d346aef735
// forLoopPushToForEachPointFreeCallee returns f when yielded is a flow applying
// a single step f to the loop variable x, as in `f(x)`, `pipe(x, f)` or
// `x.pipe(f)`, and f is a plain identifier with a single call signature taking
// at most one parameter, so Effect.forEach passing (x, index) cannot reach a
// parameter the loop never filled.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`). Go's `callee != nil` and `flow == nil`
// checks drop: the callee is non-optional and `longest_piping_flow_at` returns a flow for any expression.
pub(crate) fn for_loop_push_to_for_each_point_free_callee(
    tp: &mut TypeParser<'_>,
    binding: P<Node>,
    yielded: P<Node>,
) -> Option<P<Node>> {
    if binding.kind() != Kind::Identifier {
        return None;
    }
    let binding_symbol = tp.get_symbol_at_location(binding)?;
    let is_binding = |tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| -> bool {
        subject.node().is_some_and(|node| {
            node.kind() == Kind::Identifier
                && tp.get_symbol_at_location(node) == Some(binding_symbol)
        })
    };
    let is_plain_callee = |tp: &mut TypeParser<'_>,
                           transformation: &PipingFlowTransformation|
     -> bool {
        let callee = transformation.callee();
        match transformation.kind() {
            TransformationKind::Call | TransformationKind::Pipe | TransformationKind::Pipeable => {}
            _ => return false,
        }
        callee.kind() == Kind::Identifier
            && transformation.args().len() == 0
            && transformation.type_arguments().is_none_or(|arguments| arguments.nodes().is_empty())
            && tp.get_symbol_at_location(callee) != Some(binding_symbol)
    };
    let flow = tp.longest_piping_flow_at(yielded, false)?;
    if flow.node() != ast::skip_parentheses(yielded)
        || !PipingFlow::matches_exactly(Some(&flow), tp, &is_binding, &[&is_plain_callee])
    {
        return None;
    }
    if ast::is_optional_chain(flow.node()) {
        return None;
    }
    let callee = flow.transformations().at(0).callee();
    let callee_type = tp.get_type_at_location(callee)?;
    let signatures = tp.checker().get_signatures_of_type_exported(callee_type, SignatureKind::Call);
    if signatures.len() != 1
        || signatures[0].has_rest_parameter()
        || signatures[0].parameters().len() > 1
    {
        return None;
    }
    Some(callee)
}
