use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, SourceFile, Symbol};
use ets_shim::checker::SignatureKind;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;
use ets_shim::scanner;
use ets_typeparser::{PipingFlow, PipingFlowSubject, PipingFlowTransformation, TypeParser};
use rustc_hash::FxHashSet;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_FOR_EACH_EXPRESSES_THIS_FOR_OF_LOOP_THAT_PUSHES_YIELDED_RESULTS_INTO_AN_ACCUMULATOR_MORE_DIRECTLY_AND_MAKES_ORDERING_AND_CONCURRENCY_EXPLICIT_EFFECT_FOR_LOOP_PUSH_TO_FOR_EACH;

// go: internal/fixables/for_loop_push_to_for_each.go:18 ForLoopPushToForEachFix 05b449484297cc3e
pub static FOR_LOOP_PUSH_TO_FOR_EACH_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "forLoopPushToForEach",
    description: "Replace for-of push loop with Effect.forEach",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["forLoopPushToForEach_fix"],
    run: run_for_loop_push_to_for_each_fix,
});

// go: internal/fixables/for_loop_push_to_for_each.go:28 runForLoopPushToForEachFix bcad9be5dbfede6a
fn run_for_loop_push_to_for_each_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_for_loop_push_to_for_each_exported(&mut ctx.type_parser, sf) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let for_of = m.loop_.as_for_in_or_of_statement();
        let binding = for_of.initializer().as_variable_declaration_list().declarations().nodes()[0]
            .as_variable_declaration()
            .name();
        let yielded = m
            .flow
            .subject()
            .node()
            .expect("a yield* subject")
            .as_yield_expression()
            .expression()
            .expect("a yield* operand");
        let accumulator = m
            .declaration
            .as_variable_statement()
            .declaration_list()
            .as_variable_declaration_list()
            .declarations()
            .nodes()[0]
            .as_variable_declaration();
        // The fix moves binding and yielded into an arrow function, where a
        // yield of the generator cannot go, and turns the accumulator into the
        // result of Effect.forEach, which later mutations would no longer extend.
        if contains_generator_yield(binding) || contains_generator_yield(yielded) || {
            let accumulator_symbol = ctx.type_parser.get_symbol_at_location(accumulator.name());
            for_loop_push_to_for_each_mutated_after_loop(
                &mut ctx.type_parser,
                m.loop_,
                accumulator_symbol,
            )
        } {
            continue;
        }
        let for_each = effect_module_member("forEach");
        let Some(reference) = resolve_module_member(
            ctx,
            m.loop_,
            &ets_typeparser::find_effect_module_reference(Some(sf)),
            &for_each,
        ) else {
            continue;
        };

        // The fix deletes one statement and replaces another, the loop; the
        // piping flow edits only rewrite steps inside a flow, so none applies.
        // The replacement is written as text: the printer drops the parentheses
        // of a synthesized single-identifier arrow parameter.
        let mut callee = reference.path.string();
        if reference.via_module {
            callee += ".";
            callee += for_each.name;
        }
        let point_free_callee =
            for_loop_push_to_for_each_point_free_callee(&mut ctx.type_parser, binding, yielded);
        let mut callback = for_loop_push_to_for_each_node_text(sf, point_free_callee);
        if point_free_callee.is_none() {
            let mut body = for_loop_push_to_for_each_node_text(sf, Some(yielded));
            // A concise arrow body starting with `{` would parse as a block.
            if body.starts_with('{') {
                body = "(".to_string() + &body + ")";
            }
            callback = "(".to_string()
                + &for_loop_push_to_for_each_node_text(sf, Some(binding))
                + ") => "
                + &body;
        }
        let mut declaration = "const ".to_string()
            + &for_loop_push_to_for_each_node_text(sf, Some(accumulator.name()));
        if accumulator.type_().is_some() {
            declaration += ": ";
            declaration += &for_loop_push_to_for_each_node_text(sf, accumulator.type_());
        }
        let mut replacement = declaration
            + " = yield* "
            + &callee
            + "("
            + &for_loop_push_to_for_each_node_text(sf, Some(for_of.expression()))
            + ", "
            + &callback
            + ")";
        if for_loop_push_to_for_each_needs_semicolon(sf, m.loop_) {
            replacement += ";";
        }

        let ctx_ref: &Context<'_, '_> = ctx;
        let declaration_node = m.declaration;
        let loop_ = m.loop_;
        if let Some(action) = ctx_ref.new_fix_action(FixAction {
            description: "Replace with Effect.forEach".to_string(),
            run: Box::new(move |tracker| {
                tracker.delete(sf, declaration_node);
                tracker.replace_range_with_text(
                    sf,
                    lsproto::Range {
                        start: ctx_ref.byte_pos_to_lsp_position(scanner::get_token_pos_of_node(
                            loop_, sf, false,
                        )),
                        end: ctx_ref
                            .byte_pos_to_lsp_position(loop_.end().expect_source("a parsed loop")),
                    },
                    &replacement,
                );
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/for_loop_push_to_for_each.go:97 forLoopPushToForEachMutatingMethods 98a183b4d25fcfac
// forLoopPushToForEachMutatingMethods are the Array methods that change the
// array they are called on.
pub(crate) static FOR_LOOP_PUSH_TO_FOR_EACH_MUTATING_METHODS: LazyLock<FxHashSet<&'static str>> =
    LazyLock::new(|| {
        ["push", "pop", "shift", "unshift", "splice", "sort", "reverse", "fill", "copyWithin"]
            .into_iter()
            .collect()
    });

// go: internal/fixables/for_loop_push_to_for_each.go:105 forLoopPushToForEachMutatedAfterLoop 42d2cf6beeb9a9df
// forLoopPushToForEachMutatedAfterLoop reports whether a statement after loop
// in the same block calls a mutating Array method on the accumulator, writes
// one of its elements or its length, or deletes one of its elements.
//
// Rust: Go's nil symbol is `None`, for which `references_to_symbol` returns no reference.
pub(crate) fn for_loop_push_to_for_each_mutated_after_loop(
    tp: &mut TypeParser<'_>,
    loop_: P<Node>,
    symbol: Option<P<Symbol>>,
) -> bool {
    let loop_end = loop_.end().expect_source("a parsed loop");
    for reference in tp.references_to_symbol(loop_.parent(), symbol) {
        if reference.pos().expect_source("a parsed reference") < loop_end {
            continue;
        }
        let parent = reference.parent().expect("a reference has a parent");
        match parent.kind() {
            Kind::PropertyAccessExpression => {
                let access = parent.as_property_access_expression();
                if access.expression() != reference {
                    continue;
                }
                let member = access.name().text();
                if FOR_LOOP_PUSH_TO_FOR_EACH_MUTATING_METHODS.contains(member)
                    || member == "length" && ast::is_write_access(parent)
                {
                    return true;
                }
            }
            Kind::ElementAccessExpression => {
                if parent.as_element_access_expression().expression() != reference {
                    continue;
                }
                if ast::is_write_access(parent)
                    || parent.parent().is_some_and(|p| p.kind() == Kind::DeleteExpression)
                {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

// go: internal/fixables/for_loop_push_to_for_each.go:135 containsGeneratorYield 7a526bf74e00d5db
// containsGeneratorYield reports whether node contains a yield expression of
// the enclosing generator, which could not move into an arrow function.
pub(crate) fn contains_generator_yield(node: P<Node>) -> bool {
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

// go: internal/fixables/for_loop_push_to_for_each.go:156 forLoopPushToForEachPointFreeCallee 1fe4b5fbac42fc18
// forLoopPushToForEachPointFreeCallee returns f when yielded is a flow applying
// a single step f to the loop variable x, as in `f(x)`, `pipe(x, f)` or
// `x.pipe(f)`, and f is a plain identifier with a single call signature taking
// at most one parameter, so Effect.forEach passing (x, index) cannot reach a
// parameter the loop never filled. It returns nil when the fix has to keep an
// arrow around yielded.
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
    // No arguments leaves only call, pipe and pipeable steps: every other kind
    // keeps at least one argument besides the subject.
    let is_plain_callee = |tp: &mut TypeParser<'_>,
                           transformation: &PipingFlowTransformation|
     -> bool {
        let callee = transformation.callee();
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

// go: internal/fixables/for_loop_push_to_for_each.go:199 forLoopPushToForEachNeedsSemicolon cc5f1eee77e78853
// forLoopPushToForEachNeedsSemicolon reports whether the declaration replacing
// loop must end with a semicolon. A for statement delimits itself, while the
// declaration relies on automatic semicolon insertion: it keeps the semicolon
// that ended a statement body, and adds one when the next statement starts on
// the same line or with a token that would continue the expression.
pub(crate) fn for_loop_push_to_for_each_needs_semicolon(sf: P<SourceFile>, loop_: P<Node>) -> bool {
    let text = sf.text();
    let end = loop_.end().expect_source("a parsed loop");
    if text[..end as usize].ends_with(';') {
        return true;
    }
    let next = scanner::skip_trivia(text, end) as usize;
    if next >= text.len() || text.as_bytes()[next] == b'}' || text.as_bytes()[next] == b';' {
        return false;
    }
    if !text[end as usize..next].contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        return true;
    }
    b"([`+-/<".contains(&text.as_bytes()[next])
}

// go: internal/fixables/for_loop_push_to_for_each.go:215 forLoopPushToForEachNodeText e5253df3ccad30da
pub(crate) fn for_loop_push_to_for_each_node_text(
    sf: P<SourceFile>,
    node: Option<P<Node>>,
) -> String {
    let Some(node) = node else {
        return String::new();
    };
    scanner::get_source_text_of_node_from_source_file(sf, node, false)
}
