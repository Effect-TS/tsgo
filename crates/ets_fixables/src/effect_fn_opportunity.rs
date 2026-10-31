use std::sync::{Arc, LazyLock};

use ets_etscore::{
    EFFECT_FN_INFERRED_SPAN, EFFECT_FN_NO_SPAN, EFFECT_FN_SPAN, EFFECT_FN_SUGGESTED_SPAN,
    EFFECT_FN_UNTRACED, ResolvedEffectPluginOptions,
};
use ets_fixable::{Context, FixAction, Fixable};
use ets_go::strconv;
use ets_readonly::Slice;
use ets_rewriter::Tracker;
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, SourceFile};
use ets_shim::astnav;
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;
use ets_shim::scanner;
use ets_typeparser::EffectFnOpportunityResult;
use rustc_hash::FxHashSet;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::THIS_EXPRESSION_CAN_BE_REWRITTEN_IN_THE_REUSABLE_FUNCTION_FORM_0_EFFECT_EFFECT_FN_OPPORTUNITY;

// go: internal/fixables/effect_fn_opportunity.go:22 EffectFnOpportunityFix 2fa93ca779d87146
pub static EFFECT_FN_OPPORTUNITY_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "effectFnOpportunity",
    description: "Convert to Effect.fn",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec![
        "effectFnOpportunity_toEffectFnWithSpan",
        "effectFnOpportunity_toEffectFnUntraced",
        "effectFnOpportunity_toEffectFnNoSpan",
        "effectFnOpportunity_toEffectFnSpanInferred",
        "effectFnOpportunity_toEffectFnSpanSuggested",
    ],
    run: run_effect_fn_opportunity_fix,
});

// go: internal/fixables/effect_fn_opportunity.go:36 runEffectFnOpportunityFix 95608618bd5e9ec9
//
// Rust: Go's `effectConfig.EffectFnIncludes(...)` (a nil-receiver method) is
// `ResolvedEffectPluginOptions::effect_fn_includes(effect_config, ...)`; the result is shared by the actions'
// closures through its `Arc`.
fn run_effect_fn_opportunity_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let effect_config = ctx.options;

    let matches = ets_rules::analyze_effect_fn_opportunity(&mut ctx.type_parser, sf);

    let mut result: Option<Arc<EffectFnOpportunityResult>> = None;
    for m in matches {
        let diag_range = m.location;
        if diag_range.intersects(ctx.span) || ctx.span.contained_by(diag_range) {
            result = Some(m.result);
            break;
        }
    }
    let Some(result) = result else {
        return Vec::new();
    };
    let is_func_decl = result.target_node().kind() == Kind::FunctionDeclaration;
    let ctx = &*ctx;
    let includes =
        |variant: &str| ResolvedEffectPluginOptions::effect_fn_includes(effect_config, variant);

    let mut actions = Vec::new();

    // Fix 1: toEffectFnWithSpan - available when explicit withSpan expression exists
    if includes(EFFECT_FN_SPAN) && result.explicit_trace_expression().is_some() {
        // Remove withSpan from pipe args (it's the last one)
        let mut pipe_args = result.pipe_arguments();
        if pipe_args.len() > 0 {
            pipe_args = pipe_args.slice(0, pipe_args.len() - 1);
        }
        let pipe_args = pipe_args.copy();
        let result = Arc::clone(&result);
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Convert to Effect.fn (with span from withSpan)".to_string(),
            run: Box::new(move |tracker| {
                let trace = scanner::get_text_of_node(result.explicit_trace_expression().unwrap());
                effect_fn_build_replacement(
                    ctx,
                    tracker,
                    sf,
                    &result,
                    "fn",
                    &trace,
                    ets_readonly::new(pipe_args.as_deref()),
                    is_func_decl,
                );
            }),
        }) {
            actions.push(action);
        }
    }

    // Fix 2: toEffectFnUntraced - available when gen opportunity (generator function exists)
    if includes(EFFECT_FN_UNTRACED) && result.generator_function().is_some() {
        let result = Arc::clone(&result);
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Convert to Effect.fnUntraced".to_string(),
            run: Box::new(move |tracker| {
                effect_fn_build_replacement(
                    ctx,
                    tracker,
                    sf,
                    &result,
                    "fnUntraced",
                    "",
                    result.pipe_arguments(),
                    is_func_decl,
                );
            }),
        }) {
            actions.push(action);
        }
    }

    // Fix 3: toEffectFnNoSpan - available when no-span variant is enabled
    if includes(EFFECT_FN_NO_SPAN) {
        let result = Arc::clone(&result);
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Convert to Effect.fn (no span)".to_string(),
            run: Box::new(move |tracker| {
                effect_fn_build_replacement(
                    ctx,
                    tracker,
                    sf,
                    &result,
                    "fn",
                    "",
                    result.pipe_arguments(),
                    is_func_decl,
                );
            }),
        }) {
            actions.push(action);
        }
    }

    // Fix 4: toEffectFnSpanInferred - available when no explicit withSpan and inferred trace name exists
    if includes(EFFECT_FN_INFERRED_SPAN)
        && result.explicit_trace_expression().is_none()
        && !result.inferred_trace_name().is_empty()
    {
        let description = format!("Convert to Effect.fn(\"{}\")", result.inferred_trace_name());
        let result = Arc::clone(&result);
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                let trace = strconv::quote(result.inferred_trace_name());
                effect_fn_build_replacement(
                    ctx,
                    tracker,
                    sf,
                    &result,
                    "fn",
                    &trace,
                    result.pipe_arguments(),
                    is_func_decl,
                );
            }),
        }) {
            actions.push(action);
        }
    }

    // Fix 5: toEffectFnSpanSuggested - available when no explicit withSpan, has suggested trace name,
    // and (inferred-span is not enabled OR suggested name differs from inferred name)
    if includes(EFFECT_FN_SUGGESTED_SPAN)
        && result.explicit_trace_expression().is_none()
        && !result.suggested_trace_name().is_empty()
        && (!includes(EFFECT_FN_INFERRED_SPAN)
            || result.suggested_trace_name() != result.inferred_trace_name())
    {
        let description = format!("Convert to Effect.fn(\"{}\")", result.suggested_trace_name());
        let result = Arc::clone(&result);
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                let trace = strconv::quote(result.suggested_trace_name());
                effect_fn_build_replacement(
                    ctx,
                    tracker,
                    sf,
                    &result,
                    "fn",
                    &trace,
                    result.pipe_arguments(),
                    is_func_decl,
                );
            }),
        }) {
            actions.push(action);
        }
    }

    actions
}

// go: internal/fixables/effect_fn_opportunity.go:137 effectFnBuildReplacement 42ede7dded3d7430
// effectFnBuildReplacement edits around the original body rather than printing a
// cloned AST. The body and all surrounding declarations/properties retain their
// comments, literal spelling, whitespace, and semicolon style.
#[expect(clippy::too_many_arguments, reason = "Go's signature")]
fn effect_fn_build_replacement(
    ctx: &Context<'_, '_>,
    tracker: &mut Tracker,
    sf: P<SourceFile>,
    result: &EffectFnOpportunityResult,
    variant: &str,
    trace: &str,
    pipe_args: Slice<'_, P<Node>>,
    is_func_decl: bool,
) {
    let target = result.target_node();
    let mut body = ets_typeparser::get_function_like_body(target);
    if result.has_gen_body()
        && let Some(generator_function) = result.generator_function()
    {
        body = generator_function.body();
    }
    let Some(body) = body else {
        return;
    };

    let text = sf.text();
    let start = scanner::get_token_pos_of_node(target, sf, false);
    let body_start = scanner::get_token_pos_of_node(body, sf, false);
    let mut prefix =
        format!("{}.{variant}", ets_typeparser::find_effect_module_reference(Some(sf)).string());
    if !trace.is_empty() {
        prefix += "(";
        prefix += trace;
        prefix += ")";
    }
    prefix += "(";
    let mut suffix = String::new();

    if !result.has_gen_body() && target.kind() == Kind::FunctionExpression {
        // Keep the original function expression's signature verbatim.
        prefix += &text[start as usize..body_start as usize];
    } else {
        prefix += "function";
        if result.has_gen_body() {
            prefix += "*";
        }
        let signature = effect_fn_signature_range(sf, target);
        let signature_pos = signature.pos().expect_source("a source range");
        let signature_end = signature.end().expect_source("a source range");
        let mut signature_text = text[signature_pos as usize..signature_end as usize].to_string();
        if !signature_text.starts_with('(') && !signature_text.starts_with('<') {
            signature_text = format!("({signature_text})");
        }
        let mut comment_start = start;
        if is_func_decl && let Some(modifiers) = target.modifiers() {
            comment_start = modifiers.end().expect_source("parsed modifiers");
        }
        prefix += &(effect_fn_comments(sf, comment_start, signature_pos, None) + &signature_text);
        let comments = effect_fn_comments(sf, signature_end, body_start, None);
        if !comments.is_empty() {
            prefix += &comments;
        } else {
            prefix += " ";
        }
        if body.kind() != Kind::Block {
            prefix += "{ return ";
            suffix.push_str(" }");
        }
    }

    if is_func_decl {
        let decl = target.as_function_declaration();
        let mut declaration_prefix = String::new();
        if let Some(modifiers) = decl.modifiers() {
            let modifiers_end = modifiers.end().expect_source("parsed modifiers");
            declaration_prefix = text[start as usize..modifiers_end as usize].to_string() + " ";
        }
        prefix = format!(
            "{declaration_prefix}const {} = {prefix}",
            scanner::get_text_of_node(
                decl.name().expect("a reported function declaration has a name")
            )
        );
    }

    let body_end = body.end().expect_source("a parsed body");
    let target_end = target.end().expect_source("a parsed target");
    let mut cursor = body_end;
    for (_, &arg) in pipe_args.all() {
        // Include both leading and trailing trivia, including line comments before
        // the next comma/closing parenthesis. Never reprint the argument itself.
        let arg_pos = arg.pos().expect_source("a parsed argument");
        let arg_end = arg.end().expect_source("a parsed argument");
        let end = scanner::skip_trivia(text, arg_end);
        let start = scanner::get_token_pos_of_node(arg, sf, false);
        suffix.push(',');
        suffix.push_str(&effect_fn_comments(sf, cursor, arg_pos, None));
        suffix.push_str(&text[arg_pos as usize..start as usize]);
        suffix
            .push_str(&effect_fn_pipe_argument_text(arg, &text[start as usize..arg_end as usize]));
        suffix.push_str(&text[arg_end as usize..end as usize]);
        cursor = end;
    }
    let mut moved_trace = None;
    if !trace.is_empty() {
        moved_trace = result.explicit_trace_expression();
    }
    suffix.push_str(&effect_fn_comments(sf, cursor, target_end, moved_trace));
    suffix.push(')');
    if is_func_decl {
        suffix.push(';');
    }

    tracker.replace_range_with_text(
        sf,
        lsproto::Range {
            start: ctx.byte_pos_to_lsp_position(start),
            end: ctx.byte_pos_to_lsp_position(body_start),
        },
        &prefix,
    );
    tracker.replace_range_with_text(
        sf,
        lsproto::Range {
            start: ctx.byte_pos_to_lsp_position(body_end),
            end: ctx.byte_pos_to_lsp_position(target_end),
        },
        &suffix,
    );
}

// go: internal/fixables/effect_fn_opportunity.go:240 effectFnPipeArgumentText af209f3350f03477
// effectFnPipeArgumentText prevents Effect.fn's additional function arguments
// from reaching a bare pipeable's optional parameters. Factory calls already
// produce data-last pipeables and can be retained as written.
fn effect_fn_pipe_argument_text(arg: P<Node>, text: &str) -> String {
    if ast::skip_parentheses(arg).kind() == Kind::CallExpression {
        return text.to_string();
    }

    // Avoid capturing references such as _.ignore in the new arrow's scope.
    let mut identifiers: FxHashSet<&'static str> = FxHashSet::default();
    fn visit(node: P<Node>, identifiers: &mut FxHashSet<&'static str>) -> bool {
        if node.kind() == Kind::Identifier {
            identifiers.insert(node.text());
        }
        node.for_each_child(&mut |child| visit(child, identifiers));
        false
    }
    visit(arg, &mut identifiers);
    let mut parameter = "_".to_string();
    let mut suffix = 1;
    while identifiers.contains(parameter.as_str()) {
        parameter = format!("_{suffix}");
        suffix += 1;
    }

    let text = match arg.kind() {
        // These expressions can be used directly as a callee.
        Kind::Identifier
        | Kind::PropertyAccessExpression
        | Kind::ElementAccessExpression
        | Kind::ParenthesizedExpression => text.to_string(),
        _ => format!("({text})"),
    };
    format!("{parameter} => {text}({parameter})")
}

// go: internal/fixables/effect_fn_opportunity.go:274 effectFnSignatureRange 29da66f2ca25dba8
// effectFnSignatureRange locates the complete parameter/type-parameter spelling,
// including comments and defaults. Bare arrow parameters need parentheses when
// moved to a function expression.
fn effect_fn_signature_range(sf: P<SourceFile>, target: P<Node>) -> TextRange {
    let text = sf.text();
    let params = ets_typeparser::get_function_like_parameters(target)
        .expect("a function-like node has a parameter list");
    let mut start = params.pos().expect_source("parsed parameters");
    let mut end = params.end().expect_source("parsed parameters");
    if start > 0 && text.as_bytes()[start as usize - 1] == b'(' {
        start -= 1;
        end = scanner::skip_trivia(text, end) + 1; // closing parenthesis
    } else {
        return TextRange::new(scanner::skip_trivia(text, start), end);
    }
    if let Some(type_params) = ets_typeparser::get_function_like_type_parameters(target) {
        start = type_params.pos().expect_source("parsed type parameters") - 1; // opening angle bracket
    }
    TextRange::new(start, end)
}

// go: internal/fixables/effect_fn_opportunity.go:292 effectFnComments f46f6ce346a0400c
// effectFnComments retains comments from discarded wrapper syntax, such as the
// outer return statement. Arguments and the function body are copied separately.
fn effect_fn_comments(sf: P<SourceFile>, start: u32, end: u32, moved: Option<P<Node>>) -> String {
    let text = &sf.text()[start as usize..end as usize];
    let mut s = scanner::Scanner::new();
    s.set_skip_trivia(false);
    s.set_text(text);
    let mut comments = String::new();
    let mut trivia_start: u32 = 0;
    let mut has_comment = false;
    let mut token = s.scan();
    while token != Kind::EndOfFile {
        if let Some(moved) = moved {
            let moved_end = moved.end().expect_source("a parsed node");
            if start + s.token_start() >= scanner::get_token_pos_of_node(moved, sf, false)
                && start + s.token_start() < moved_end
            {
                if has_comment {
                    comments.push_str(&text[trivia_start as usize..s.token_start() as usize]);
                }
                trivia_start = moved_end.min(end) - start;
                s.reset_pos(trivia_start);
                has_comment = false;
                token = s.scan();
                continue;
            }
        }
        match token {
            Kind::SingleLineCommentTrivia | Kind::MultiLineCommentTrivia => {
                comments.push_str(&text[trivia_start as usize..s.token_end() as usize]);
                trivia_start = s.token_end();
                has_comment = true;
            }
            Kind::WhitespaceTrivia | Kind::NewLineTrivia => {
                // Whitespace is retained with the adjacent comment.
            }
            _ => {
                if has_comment {
                    comments.push_str(&text[trivia_start as usize..s.token_start() as usize]);
                }
                // Use the parsed token's extent for contextual tokens (regexes and
                // template tails), so comment-like text inside literals is not moved.
                let original = astnav::get_token_at_position(sf, start + s.token_start());
                trivia_start = s.token_end();
                let original_end = original.end().expect_source("a parsed token");
                if original_end > start + trivia_start {
                    trivia_start = original_end.min(end) - start;
                    s.reset_pos(trivia_start);
                }
                has_comment = false;
            }
        }
        token = s.scan();
    }
    if has_comment {
        comments.push_str(&text[trivia_start as usize..]);
    }
    comments
}
