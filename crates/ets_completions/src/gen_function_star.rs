use std::sync::LazyLock;

use ets_completion::{Completion, Context};
use ets_shim::checker::SignatureKind;
use ets_shim::lsproto;

use crate::*;

// go: internal/completions/gen_function_star.go:11 genFunctionStar
// genFunctionStar provides a completion for `gen(function*(){})` when dot-accessing
// an object that has a callable `gen` property (e.g., `Effect.gen`, `Micro.gen`).
pub(crate) static GEN_FUNCTION_STAR: LazyLock<Completion> = LazyLock::new(|| Completion {
    name: "genFunctionStar",
    description: "Provides gen(function*(){}) completion when accessing an object with a callable gen property",
    run: run_gen_function_star,
});

// go: internal/completions/gen_function_star.go:17 runGenFunctionStar
//
// Rust: Go's `ch` is `ctx.checker()` (the parser's checker, see `ets_completion::Context`).
fn run_gen_function_star(ctx: &mut Context<'_, '_>) -> Vec<lsproto::CompletionItem> {
    let Some(result) =
        ets_completion::parse_accessed_expression_for_completion(ctx.source_file, ctx.position)
    else {
        return Vec::new();
    };

    let tp = &mut ctx.type_parser;

    let Some(t) = tp.get_type_at_location(result.accessed_object) else {
        return Vec::new();
    };

    let Some(gen_symbol) = tp.checker().get_property_of_type_exported(t, "gen") else {
        return Vec::new();
    };

    // Rust: tsrs's `Option` result is never `None` here (Go's result is never nil).
    let Some(gen_type) =
        tp.checker().get_type_of_symbol_at_location(gen_symbol, Some(result.accessed_object))
    else {
        return Vec::new();
    };
    let call_sigs = tp.checker().get_signatures_of_type(gen_type, SignatureKind::Call);
    if call_sigs.is_empty() {
        return Vec::new();
    }

    // Replacement span: from after the dot to the cursor position
    // This matches the reference: span starts at accessedObject.end + 1 (after the dot)
    let accessed_end = result.accessed_object.end().expect_source("a node of the file");
    let span_start = accessed_end + 1;
    let span_length = (ctx.position as i64 - span_start as i64).max(0) as u32;

    let replacement_range = byte_span_to_range(ctx, span_start, span_length);
    let sort_text = "11";

    vec![make_completion_item(
        "gen(function*(){})",
        "gen(function*(){${0}})",
        sort_text,
        replacement_range,
        None,
    )]
}
