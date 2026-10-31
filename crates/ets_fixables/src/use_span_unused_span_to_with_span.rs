use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::P;
use ets_shim::ast::Node;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::{lsproto, scanner};

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_EFFECT_USE_SPAN_CALLBACK_NEVER_USES_THE_SPAN_IT_RECEIVES_EFFECT_USE_SPAN_DOES_NOT_MAKE_THAT_SPAN_THE_PARENT_OF_SPANS_CREATED_INSIDE_THE_CALLBACK_SO_THEY_WILL_NOT_NEST_UNDER_IT_EFFECT_WITH_SPAN_DOES_AND_REMOVES_PARENT_SPAN_FROM_THE_REQUIREMENTS_EFFECT_USE_SPAN_UNUSED_SPAN_TO_WITH_SPAN;

// go: internal/fixables/use_span_unused_span_to_with_span.go:16 UseSpanUnusedSpanToWithSpanFix c1dbedceb73059e1
pub static USE_SPAN_UNUSED_SPAN_TO_WITH_SPAN_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "useSpanUnusedSpanToWithSpan",
    description: "Use Effect.withSpan",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["useSpanUnusedSpanToWithSpan_fix"],
    run: run_use_span_unused_span_to_with_span_fix,
});

// go: internal/fixables/use_span_unused_span_to_with_span.go:30 runUseSpanUnusedSpanToWithSpanFix 100a6bc1af5b1427
// runUseSpanUnusedSpanToWithSpanFix rewrites `R.useSpan(name, options?, () => body)`
// to `body.pipe(R.withSpan(name, options?))`. The useSpan call is not a piping
// flow step, so no flow edit expresses this; it edits the text around the body
// so the body keeps its original formatting and inner comments.
fn run_use_span_unused_span_to_with_span_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_use_span_unused_span_to_with_span(&mut ctx.type_parser, source_file)
    {
        let (Some(receiver), Some(body)) = (m.receiver, m.body) else {
            continue;
        };
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let sf = m.source_file;
        let parenthesize = receiver_needs_parentheses(body);
        if replacement_continues_previous_statement(sf, m.call, body, parenthesize) {
            continue;
        }
        let node_text = |node: P<Node>| -> &'static str {
            &sf.text()[scanner::get_token_pos_of_node(node, sf, false) as usize
                ..node.end().expect_source("a parsed node") as usize]
        };
        let mut prefix = String::new();
        let mut suffix = String::new();
        if parenthesize {
            prefix.push('(');
            suffix.push(')');
        }
        suffix.push_str(".pipe(");
        suffix.push_str(node_text(receiver));
        suffix.push_str(".withSpan(");
        suffix.push_str(node_text(m.name));
        if let Some(options) = m.options {
            suffix.push_str(", ");
            suffix.push_str(node_text(options));
        }
        suffix.push_str("))");

        let call = m.call;
        let ctx_ref: &Context<'_, '_> = ctx;
        if let Some(action) = ctx_ref.new_fix_action(FixAction {
            description: "Use Effect.withSpan".to_string(),
            run: Box::new(move |tracker| {
                tracker.replace_range_with_text(
                    sf,
                    lsproto::Range {
                        start: ctx_ref.byte_pos_to_lsp_position(scanner::get_token_pos_of_node(
                            call, sf, false,
                        )),
                        end: ctx_ref.byte_pos_to_lsp_position(scanner::get_token_pos_of_node(
                            body, sf, false,
                        )),
                    },
                    &prefix,
                );
                tracker.replace_range_with_text(
                    sf,
                    lsproto::Range {
                        start: ctx_ref
                            .byte_pos_to_lsp_position(body.end().expect_source("a parsed body")),
                        end: ctx_ref
                            .byte_pos_to_lsp_position(call.end().expect_source("a parsed call")),
                    },
                    &suffix,
                );
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}
