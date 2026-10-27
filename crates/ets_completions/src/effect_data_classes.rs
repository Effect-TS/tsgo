use std::sync::LazyLock;

use ets_completion::{Completion, Context};
use ets_shim::lsproto;

use crate::*;

// go: internal/completions/effect_data_classes.go:14 effectDataClasses
// effectDataClasses provides completion items for Data.TaggedError and Data.TaggedClass
// when the cursor is in the extends clause of a class declaration.
// For example, typing `class Foo extends Data.|` will suggest `TaggedError("Foo")` and `TaggedClass("Foo")`.
pub(crate) static EFFECT_DATA_CLASSES: LazyLock<Completion> = LazyLock::new(|| Completion {
    name: "effectDataClasses",
    description: "Provides Data.TaggedError and Data.TaggedClass completions in extends clauses",
    run: run_effect_data_classes,
});

// go: internal/completions/effect_data_classes.go:20 runEffectDataClasses
fn run_effect_data_classes(ctx: &mut Context<'_, '_>) -> Vec<lsproto::CompletionItem> {
    let Some(data) =
        ets_completion::parse_data_for_extends_class_completion(ctx.source_file, ctx.position)
    else {
        return Vec::new();
    };

    let data_reference = ets_typeparser::find_module_reference(ctx.source_file, "Data");
    let data_identifier = data_reference.string();
    let accessed_text = data.accessed_object_text();
    let is_fully_qualified = data_reference.matches_node(data.accessed_object);
    let class_name = data.class_name_text();

    // Build replacement range from byte offsets
    let replacement_range =
        byte_span_to_range(ctx, data.replacement_start, data.replacement_length);

    let sort_text = "11";
    let mut items = Vec::new();

    // Data.TaggedError
    if is_fully_qualified
        || ctx
            .type_parser
            .is_node_reference_to_effect_data_module_api(data.accessed_object, "TaggedError")
    {
        let error_key = compute_deterministic_key(
            ctx.program,
            &mut ctx.type_parser,
            ctx.source_file,
            &class_name,
            "error",
        );
        let insert_text = if is_fully_qualified {
            format!(r#"{data_identifier}.TaggedError("{error_key}")<{{${{0}}}}>{{}}"#)
        } else {
            format!(r#"TaggedError("{error_key}")<{{${{0}}}}>{{}}"#)
        };
        items.push(make_extends_completion_item(
            &accessed_text,
            &format!(r#"TaggedError("{class_name}")"#),
            &insert_text,
            sort_text,
            replacement_range,
        ));
    }

    // Data.TaggedClass
    if is_fully_qualified
        || ctx
            .type_parser
            .is_node_reference_to_effect_data_module_api(data.accessed_object, "TaggedClass")
    {
        let tag_key = compute_deterministic_key(
            ctx.program,
            &mut ctx.type_parser,
            ctx.source_file,
            &class_name,
            "taggedClass",
        );
        let insert_text = if is_fully_qualified {
            format!(r#"{data_identifier}.TaggedClass("{tag_key}")<{{${{0}}}}>{{}}"#)
        } else {
            format!(r#"TaggedClass("{tag_key}")<{{${{0}}}}>{{}}"#)
        };
        items.push(make_extends_completion_item(
            &accessed_text,
            &format!(r#"TaggedClass("{class_name}")"#),
            &insert_text,
            sort_text,
            replacement_range,
        ));
    }

    items
}
