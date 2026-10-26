use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::astnav;
use ets_shim::checker::Checker;
use ets_shim::compiler::Program;
use ets_shim::core::{P, TextRange};
use ets_shim::ls::lsconv::Converters;
use ets_shim::ls::lsutil::InlayHintsPreferences;
use ets_shim::lsproto;
use ets_typeparser::TypeParser;

// go: etslshooks/inlay_hints.go:16 afterInlayHints
// afterInlayHints filters out redundant return-type inlay hints on Effect.gen,
// Effect.fn, Effect.fnUntraced, and Effect.fnUntracedEager generator functions.
pub(crate) fn after_inlay_hints(
    program: &'static Program,
    c: &mut Checker,
    sf: P<SourceFile>,
    _span: TextRange,
    preferences: &InlayHintsPreferences,
    hints: Vec<lsproto::InlayHint>,
    converters: &Converters,
) -> Vec<lsproto::InlayHint> {
    let mut tp = ets_typeparser::new_type_parser(program, c);

    let effect_config = program.options().effect.get();
    if effect_config.as_deref().is_none_or(|config| !config.inlays) {
        return hints;
    }

    if !preferences.include_inlay_function_like_return_type_hints.is_true() {
        return hints;
    }

    let mut result = Vec::with_capacity(hints.len());
    for hint in hints {
        if should_omit_hint(&mut tp, sf, &hint, converters) {
            continue;
        }
        result.push(hint);
    }
    result
}

// go: etslshooks/inlay_hints.go:48 shouldOmitHint
// shouldOmitHint checks whether a single inlay hint should be suppressed
// because it is a return-type hint on an Effect generator function.
fn should_omit_hint(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    hint: &lsproto::InlayHint,
    converters: &Converters,
) -> bool {
    if hint.kind != Some(lsproto::InlayHintKind::TYPE) {
        return false;
    }

    // Convert LSP position back to raw text offset
    let offset = converters
        .from_lsp_range_to_original(
            &sf,
            lsproto::Range { start: hint.position, end: hint.position },
        )
        .pos()
        .expect_source("a converted position");

    // Find the token at offset-1 (matching the TS reference: findNodeAtPositionIncludingTrivia(sf, position - 1))
    // Rust: Go passes -1 for a hint at offset 0; tsrs positions are unsigned (a return-type hint is never there).
    let node = astnav::get_token_at_position(sf, offset.saturating_sub(1));
    if node.parent().is_none() {
        return false;
    }

    // Walk up from the token to find the CallExpression.
    // The token at offset-1 is typically the CloseParenToken of function*().
    // Its parent is the FunctionExpression, and grandparent is the CallExpression
    // (e.g. Effect.gen(function*() { ... })). For curried variants like
    // Effect.fn("name")(function*() { ... }), the outer CallExpression may be
    // one more level up. Try ancestors up to a reasonable depth.
    let mut gen_node: Option<P<Node>> = None;
    let mut gen_body: Option<P<Node>> = None;
    let mut ancestor = node.parent();
    while let Some(a) = ancestor {
        if a.kind() == Kind::CallExpression {
            (gen_node, gen_body) = match_effect_gen_call(tp, a);
            if gen_node.is_some() && gen_body.is_some() {
                break;
            }
        }
        ancestor = a.parent();
    }
    let (Some(gen_node), Some(gen_body)) = (gen_node, gen_body) else {
        return false;
    };

    // Check if the hint position falls between the close paren of the generator
    // function's parameter list and the start of the body
    let Some(close_paren) = astnav::find_child_of_kind(gen_node, Kind::CloseParenToken, sf) else {
        return false;
    };

    let body_start = astnav::get_start_of_node(gen_body, sf, false);
    offset >= close_paren.end().expect_source("a token of the file") && offset <= body_start
}

// go: etslshooks/inlay_hints.go:100 matchEffectGenCall
// matchEffectGenCall tries all four Effect generator call patterns and returns
// the generator function and body for the first match.
fn match_effect_gen_call(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> (Option<P<Node>>, Option<P<Node>>) {
    if let Some(result) = tp.effect_gen_call(node) {
        return (Some(result.generator_function()), result.body());
    }
    if let Some(result) = tp.effect_fn_call(node) {
        if result.is_generator() {
            return (result.generator_function(), result.body());
        }
    }
    (None, None)
}
