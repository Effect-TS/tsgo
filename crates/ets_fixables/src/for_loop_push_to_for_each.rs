use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::P;
use ets_shim::ast::{Node, SourceFile};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;
use ets_shim::scanner;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_FOR_EACH_EXPRESSES_THIS_FOR_OF_LOOP_THAT_PUSHES_YIELDED_RESULTS_INTO_AN_ACCUMULATOR_MORE_DIRECTLY_AND_MAKES_ORDERING_AND_CONCURRENCY_EXPLICIT_EFFECT_FOR_LOOP_PUSH_TO_FOR_EACH;

// go: internal/fixables/for_loop_push_to_for_each.go:17 ForLoopPushToForEachFix 05b449484297cc3e
pub static FOR_LOOP_PUSH_TO_FOR_EACH_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "forLoopPushToForEach",
    description: "Replace for-of push loop with Effect.forEach",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["forLoopPushToForEach_fix"],
    run: run_for_loop_push_to_for_each_fix,
});

// go: internal/fixables/for_loop_push_to_for_each.go:27 runForLoopPushToForEachFix 7d68d490a5878b86
fn run_for_loop_push_to_for_each_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_for_loop_push_to_for_each_exported(&mut ctx.type_parser, sf) {
        if !m.can_fix {
            continue;
        }
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
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

        // The replacement is written as text: the printer drops the parentheses
        // of a synthesized single-identifier arrow parameter.
        let mut callee = reference.path.string();
        if reference.via_module {
            callee += ".";
            callee += for_each.name;
        }
        let mut callback = for_loop_push_to_for_each_node_text(sf, m.point_free_callee);
        if m.point_free_callee.is_none() {
            let mut body = for_loop_push_to_for_each_node_text(sf, Some(m.yielded));
            // A concise arrow body starting with `{` would parse as a block.
            if body.starts_with('{') {
                body = "(".to_string() + &body + ")";
            }
            callback = "(".to_string()
                + &for_loop_push_to_for_each_node_text(sf, Some(m.binding))
                + ") => "
                + &body;
        }
        let mut declaration = "const ".to_string()
            + &for_loop_push_to_for_each_node_text(sf, Some(m.accumulator_name));
        if m.accumulator_type.is_some() {
            declaration += ": ";
            declaration += &for_loop_push_to_for_each_node_text(sf, m.accumulator_type);
        }
        let mut replacement = declaration
            + " = yield* "
            + &callee
            + "("
            + &for_loop_push_to_for_each_node_text(sf, Some(m.iterable))
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

// go: internal/fixables/for_loop_push_to_for_each.go:87 forLoopPushToForEachNeedsSemicolon cc5f1eee77e78853
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

// go: internal/fixables/for_loop_push_to_for_each.go:103 forLoopPushToForEachNodeText e5253df3ccad30da
pub(crate) fn for_loop_push_to_for_each_node_text(
    sf: P<SourceFile>,
    node: Option<P<Node>>,
) -> String {
    let Some(node) = node else {
        return String::new();
    };
    scanner::get_source_text_of_node_from_source_file(sf, node, false)
}
