use std::sync::LazyLock;

use ets_completion::{Completion, Context};
use ets_shim::ast::{self, Node};
use ets_shim::astnav;
use ets_shim::checker::{ContextFlags, TypeFlags};
use ets_shim::core::P;
use ets_shim::lsproto;
use ets_shim::scanner;

use crate::*;

// go: internal/completions/duration_input.go:18 durationInput
// durationInput provides completion items for Duration string literals.
// When the cursor is inside a string literal whose contextual type is a Duration union
// (containing template literals with "nanos" text), it offers snippet completions
// for all 8 duration units.
pub(crate) static DURATION_INPUT: LazyLock<Completion> = LazyLock::new(|| Completion {
    name: "durationInput",
    description: "Provides duration unit completions inside Duration string literals",
    run: run_duration_input,
});

// go: internal/completions/duration_input.go:24 durationUnits
pub(crate) static DURATION_UNITS: &[&str] =
    &["nanos", "micros", "millis", "seconds", "minutes", "hours", "days", "weeks"];

// go: internal/completions/duration_input.go:26 runDurationInput
fn run_duration_input(ctx: &mut Context<'_, '_>) -> Vec<lsproto::CompletionItem> {
    let Some(previous_token) = astnav::find_preceding_token(ctx.source_file, ctx.position) else {
        return Vec::new();
    };
    if !ast::is_string_text_containing_node(previous_token) {
        return Vec::new();
    }

    let start = scanner::get_token_pos_of_node(previous_token, ctx.source_file, false);
    let end = previous_token.end().expect_source("a token of the file");

    // The cursor must be strictly inside the string literal content,
    // or at the end position of an unterminated literal.
    let mut is_in_string = start < ctx.position && ctx.position < end;
    if ctx.position == end {
        is_in_string = ast::is_unterminated_literal(previous_token);
    }

    if !is_in_string || !ast::is_expression(previous_token) {
        return Vec::new();
    }

    let Some(contextual_type) =
        ctx.type_parser.get_contextual_type(previous_token, ContextFlags::None)
    else {
        return Vec::new();
    };
    if !contextual_type.is_union() {
        return Vec::new();
    }

    for &member in contextual_type.as_union_type().types() {
        if !member.flags().intersects(TypeFlags::TemplateLiteral) {
            continue;
        }
        let texts = member.as_template_literal_type().texts();
        if texts.len() == 2 && ets_go::strings::trim_space(texts[1]) == "nanos" {
            return build_duration_items(ctx, previous_token);
        }
    }

    Vec::new()
}

// go: internal/completions/duration_input.go:64 buildDurationItems
fn build_duration_items(ctx: &Context<'_, '_>, node: P<Node>) -> Vec<lsproto::CompletionItem> {
    // Replacement range covers the string content (between the quotes)
    let start = scanner::get_token_pos_of_node(node, ctx.source_file, false) + 1; // after opening quote
    let mut end = node.end().expect_source("a token of the file") - 1; // before closing quote
    end = end.max(start);
    let replacement_range = byte_span_to_range(ctx, start, end - start);
    let sort_text = "11";

    let mut items = Vec::with_capacity(DURATION_UNITS.len());
    for unit in DURATION_UNITS {
        items.push(make_completion_item(
            unit,
            &format!("${{0}} {unit}"),
            sort_text,
            replacement_range,
            Some(lsproto::CompletionItemKind::CONSTANT),
        ));
    }
    items
}
