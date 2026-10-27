use std::sync::LazyLock;

use ets_completion::{Completion, Context};
use ets_shim::lsproto;
use ets_typeparser::EffectMajorVersion;

use crate::*;

// go: internal/completions/schema_brand.go:13 schemaBrand 66c9381029df5cc1
// schemaBrand provides a `brand("varName")` completion when dot-accessing the
// Schema identifier inside a variable declaration. V3-only.
pub(crate) static SCHEMA_BRAND: LazyLock<Completion> = LazyLock::new(|| Completion {
    name: "schemaBrand",
    description: "Provides brand(\"varName\") completion when dot-accessing Schema in variable declarations",
    run: run_schema_brand,
});

// go: internal/completions/schema_brand.go:19 runSchemaBrand 0057a7b968aa8816
fn run_schema_brand(ctx: &mut Context<'_, '_>) -> Vec<lsproto::CompletionItem> {
    let Some(result) =
        ets_completion::parse_accessed_expression_for_completion(ctx.source_file, ctx.position)
    else {
        return Vec::new();
    };

    // V3 only
    let version = ctx.type_parser.supported_effect_version();
    if version != EffectMajorVersion::V3 {
        return Vec::new();
    }

    // Compare the accessed object with the file's reference to the Schema module
    if !ets_typeparser::find_module_reference(ctx.source_file, "Schema")
        .matches_node(result.accessed_object)
    {
        return Vec::new();
    }

    // Replacement span: from after the dot to the cursor position
    let span_start = result.accessed_object.end().expect_source("a node of the file") + 1;
    let span_length = (ctx.position as i64 - span_start as i64).max(0) as u32;

    // Find enclosing variable name
    let var_name = find_enclosing_variable_name(result.outer_node);
    if var_name.is_empty() {
        return Vec::new();
    }

    let replacement_range = byte_span_to_range(ctx, span_start, span_length);
    let sort_text = "11";

    let label = format!(r#"brand("{var_name}")"#);

    vec![make_completion_item(&label, &label, sort_text, replacement_range, None)]
}
