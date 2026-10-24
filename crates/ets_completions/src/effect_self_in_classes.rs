use std::sync::LazyLock;

use ets_completion::{Completion, Context};
use ets_shim::lsproto;
use ets_typeparser::EffectMajorVersion;

use crate::*;

// go: internal/completions/effect_self_in_classes.go:14 effectSelfInClasses
// effectSelfInClasses provides completion items for Effect.Service and Effect.Tag class constructors
// when the cursor is in the extends clause of a class declaration.
// This is a V3-only completion.
pub(crate) static EFFECT_SELF_IN_CLASSES: LazyLock<Completion> = LazyLock::new(|| Completion {
    name: "effectSelfInClasses",
    description: "Provides Effect.Service/Effect.Tag completions in extends clauses",
    run: run_effect_self_in_classes,
});

// go: internal/completions/effect_self_in_classes.go:20 runEffectSelfInClasses
fn run_effect_self_in_classes(ctx: &mut Context<'_, '_>) -> Vec<lsproto::CompletionItem> {
    let Some(data) =
        ets_completion::parse_data_for_extends_class_completion(ctx.source_file, ctx.position)
    else {
        return Vec::new();
    };

    // V3 only
    let version = ctx.type_parser.supported_effect_version();
    if version != EffectMajorVersion::V3 {
        return Vec::new();
    }

    let effect_identifier = ets_typeparser::find_module_identifier(ctx.source_file, "Effect");
    let accessed_text = data.accessed_object_text();
    let is_fully_qualified = effect_identifier == accessed_text;
    let class_name = data.class_name_text();

    // Compute deterministic tag key
    let tag_key = compute_deterministic_key(
        ctx.program,
        &mut ctx.type_parser,
        ctx.source_file,
        &class_name,
        "service",
    );

    // Build replacement range from byte offsets
    let replacement_range =
        byte_span_to_range(ctx, data.replacement_start, data.replacement_length);

    let sort_text = "11";
    let mut items = Vec::new();

    // Service: Effect.Service<ClassName>()("tagKey", {}){}
    if is_fully_qualified
        || ctx.type_parser.is_node_reference_to_effect_module_api(data.accessed_object, "Service")
    {
        let insert_text = if is_fully_qualified {
            format!(r#"{effect_identifier}.Service<{class_name}>()("{tag_key}", {{${{0}}}}){{}}"#)
        } else {
            format!(r#"Service<{class_name}>()("{tag_key}", {{${{0}}}}){{}}"#)
        };
        items.push(make_extends_completion_item(
            &accessed_text,
            &format!("Service<{class_name}>"),
            &insert_text,
            sort_text,
            replacement_range,
        ));
    }

    // Tag: Effect.Tag("tagKey")<ClassName, {}>(){}
    if is_fully_qualified
        || ctx.type_parser.is_node_reference_to_effect_module_api(data.accessed_object, "Tag")
    {
        let insert_text = if is_fully_qualified {
            format!(r#"{effect_identifier}.Tag("{tag_key}")<{class_name}, {{${{0}}}}>(){{}}"#)
        } else {
            format!(r#"Tag("{tag_key}")<{class_name}, {{${{0}}}}>(){{}}"#)
        };
        items.push(make_extends_completion_item(
            &accessed_text,
            &format!(r#"Tag("{class_name}")"#),
            &insert_text,
            sort_text,
            replacement_range,
        ));
    }

    items
}
