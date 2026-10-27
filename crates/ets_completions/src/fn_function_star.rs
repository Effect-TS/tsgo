use std::sync::LazyLock;

use ets_completion::{Completion, Context};
use ets_shim::ast::{self, Kind, Node};
use ets_shim::core::P;
use ets_shim::lsproto;
use ets_shim::scanner;

use crate::*;

// go: internal/completions/fn_function_star.go:16 fnFunctionStar aa33489aaa9f1a25
// fnFunctionStar provides completions for `fn(function*(){})` when dot-accessing
// the Effect module identifier (e.g., `Effect.fn|`). This is AST-based and does
// not require the type checker.
pub(crate) static FN_FUNCTION_STAR: LazyLock<Completion> = LazyLock::new(|| Completion {
    name: "fnFunctionStar",
    description: "Provides fn(function*(){}) completion when accessing the Effect module",
    run: run_fn_function_star,
});

// go: internal/completions/fn_function_star.go:22 runFnFunctionStar 3f44feb51e5f4354
fn run_fn_function_star(ctx: &mut Context<'_, '_>) -> Vec<lsproto::CompletionItem> {
    let Some(result) =
        ets_completion::parse_accessed_expression_for_completion(ctx.source_file, ctx.position)
    else {
        return Vec::new();
    };

    // Compare the accessed object with the file's reference to the Effect module
    if !ets_typeparser::find_effect_module_reference(ctx.source_file)
        .matches_node(result.accessed_object)
    {
        return Vec::new();
    }

    // Replacement span: from after the dot to the cursor position
    let span_start = result.accessed_object.end().expect_source("a node of the file") + 1;
    let span_length = (ctx.position as i64 - span_start as i64).max(0) as u32;

    let replacement_range = byte_span_to_range(ctx, span_start, span_length);
    let sort_text = "11";

    let mut items = Vec::new();

    // Try to find enclosing variable declaration to extract variable name
    let var_name = find_enclosing_variable_name(result.outer_node);
    if !var_name.is_empty() {
        items.push(make_completion_item(
            &format!(r#"fn("{var_name}")"#),
            &format!(r#"fn("{var_name}")(function*(${{1}}){{${{0}}}})"#),
            sort_text,
            replacement_range,
            None,
        ));
    }

    // Always add generic fn and fnUntraced
    items.extend([
        make_completion_item(
            "fn(function*(){})",
            "fn(function*(${1}){${0}})",
            sort_text,
            replacement_range,
            None,
        ),
        make_completion_item(
            "fnUntraced(function*(){})",
            "fnUntraced(function*(${1}){${0}})",
            sort_text,
            replacement_range,
            None,
        ),
    ]);

    items
}

// go: internal/completions/fn_function_star.go:74 findEnclosingVariableName 7f330f73b4e7e234
// findEnclosingVariableName walks up from the given node to find an enclosing
// VariableDeclaration and returns its identifier name, or "" if not found.
pub(crate) fn find_enclosing_variable_name(node: P<Node>) -> String {
    let Some(var_decl) = ast::find_ancestor(node, |n| n.kind() == Kind::VariableDeclaration) else {
        return String::new();
    };

    let Some(name) = var_decl.name() else {
        return String::new();
    };
    if !ast::is_identifier(name) {
        return String::new();
    }

    scanner::get_text_of_node(name)
}
