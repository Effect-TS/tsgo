use std::sync::LazyLock;

use ets_completion::{Completion, Context};
use ets_etscore::{
    BRANDED_SCHEMA_CLASS_CLASS, BRANDED_SCHEMA_CLASS_ERROR, BRANDED_SCHEMA_CLASS_OPAQUE,
    BRANDED_SCHEMA_CLASS_TAGGED_CLASS, BRANDED_SCHEMA_CLASS_TAGGED_ERROR,
    ResolvedEffectPluginOptions,
};
use ets_shim::lsproto;
use ets_shim::scanner;
use ets_typeparser::EffectMajorVersion;

use crate::*;

// go: internal/completions/effect_schema_self_in_classes.go:18 effectSchemaSelfInClasses e2a80dad4e0246c1
// effectSchemaSelfInClasses provides completion items for Schema/Model class constructors
// when the cursor is in the extends clause of a class declaration.
// For example, typing `class Foo extends Schema.|` will suggest `Class<Foo>`, `TaggedClass<Foo>`, etc.
pub(crate) static EFFECT_SCHEMA_SELF_IN_CLASSES: LazyLock<Completion> =
    LazyLock::new(|| Completion {
        name: "effectSchemaSelfInClasses",
        description: "Provides Schema/Model class constructor completions in extends clauses",
        run: run_effect_schema_self_in_classes,
    });

// go: internal/completions/effect_schema_self_in_classes.go:24 runEffectSchemaSelfInClasses 11ef76b317f87877
fn run_effect_schema_self_in_classes(ctx: &mut Context<'_, '_>) -> Vec<lsproto::CompletionItem> {
    let Some(data) =
        ets_completion::parse_data_for_extends_class_completion(ctx.source_file, ctx.position)
    else {
        return Vec::new();
    };

    let schema_reference = ets_typeparser::find_module_reference(ctx.source_file, "Schema");
    let schema_identifier = schema_reference.string();
    let accessed_text = data.accessed_object_text();
    let is_fully_qualified = schema_reference.matches_node(data.accessed_object);
    let class_name = data.class_name_text();

    let version = ctx.type_parser.supported_effect_version();
    let resolved_options = ctx.program.options().effect.get().and_then(|effect_config| {
        ets_pluginoptions::resolve_effect_plugin_options_for_source_file(
            Some(&effect_config),
            ctx.source_file.file_name(),
            &ctx.program.options().config_file_path,
            ctx.program.use_case_sensitive_file_names(),
        )
    });
    // schemaTypeArgs returns the Self type argument, followed by a brand in v4 when the kind is branded.
    let schema_type_args = |kind: &str| -> String {
        if version == EffectMajorVersion::V4
            && ResolvedEffectPluginOptions::branded_schema_classes_includes(
                resolved_options.as_ref(),
                kind,
            )
        {
            return format!("{class_name}, {{ readonly brand: unique symbol }}");
        }
        class_name.clone()
    };

    // Build replacement range from byte offsets
    let replacement_range =
        byte_span_to_range(ctx, data.replacement_start, data.replacement_length);

    let sort_text = "11";
    let mut items = Vec::new();

    // Schema.Class (both v3 and v4)
    if is_fully_qualified
        || ctx
            .type_parser
            .is_node_reference_to_effect_schema_module_api(data.accessed_object, "Class")
    {
        let schema_type_args = schema_type_args(BRANDED_SCHEMA_CLASS_CLASS);
        let insert_text = if is_fully_qualified {
            format!(
                r#"{schema_identifier}.Class<{schema_type_args}>("{class_name}")({{${{0}}}}){{}}"#
            )
        } else {
            format!(r#"Class<{schema_type_args}>("{class_name}")({{${{0}}}}){{}}"#)
        };
        items.push(make_extends_completion_item(
            &accessed_text,
            &format!("Class<{class_name}>"),
            &insert_text,
            sort_text,
            replacement_range,
        ));
    }

    // Schema.TaggedError (both v3 and v4)
    if is_fully_qualified
        || ctx
            .type_parser
            .is_node_reference_to_effect_schema_module_api(data.accessed_object, "TaggedError")
    {
        let error_key = compute_deterministic_key(
            ctx.program,
            &mut ctx.type_parser,
            ctx.source_file,
            &class_name,
            "error",
        );
        let schema_type_args = schema_type_args(BRANDED_SCHEMA_CLASS_TAGGED_ERROR);
        let insert_text = if is_fully_qualified {
            format!(
                r#"{schema_identifier}.TaggedError<{schema_type_args}>()("{error_key}", {{${{0}}}}){{}}"#
            )
        } else {
            format!(r#"TaggedError<{schema_type_args}>()("{error_key}", {{${{0}}}}){{}}"#)
        };
        items.push(make_extends_completion_item(
            &accessed_text,
            &format!("TaggedError<{class_name}>"),
            &insert_text,
            sort_text,
            replacement_range,
        ));
    }

    // Schema.TaggedClass (both v3 and v4)
    if is_fully_qualified
        || ctx
            .type_parser
            .is_node_reference_to_effect_schema_module_api(data.accessed_object, "TaggedClass")
    {
        let tag_key = compute_deterministic_key(
            ctx.program,
            &mut ctx.type_parser,
            ctx.source_file,
            &class_name,
            "taggedClass",
        );
        let schema_type_args = schema_type_args(BRANDED_SCHEMA_CLASS_TAGGED_CLASS);
        let insert_text = if is_fully_qualified {
            format!(
                r#"{schema_identifier}.TaggedClass<{schema_type_args}>()("{tag_key}", {{${{0}}}}){{}}"#
            )
        } else {
            format!(r#"TaggedClass<{schema_type_args}>()("{tag_key}", {{${{0}}}}){{}}"#)
        };
        items.push(make_extends_completion_item(
            &accessed_text,
            &format!("TaggedClass<{class_name}>"),
            &insert_text,
            sort_text,
            replacement_range,
        ));
    }

    // Schema.TaggedRequest (v3 only)
    if version == EffectMajorVersion::V3 {
        if is_fully_qualified
            || ctx.type_parser.is_node_reference_to_effect_schema_module_api(
                data.accessed_object,
                "TaggedRequest",
            )
        {
            let tag_key = compute_deterministic_key(
                ctx.program,
                &mut ctx.type_parser,
                ctx.source_file,
                &class_name,
                "taggedClass",
            );
            let insert_text = if is_fully_qualified {
                format!(
                    r#"{schema_identifier}.TaggedRequest<{class_name}>()("{tag_key}", {{${{0}}}}){{}}"#
                )
            } else {
                format!(r#"TaggedRequest<{class_name}>()("{tag_key}", {{${{0}}}}){{}}"#)
            };
            items.push(make_extends_completion_item(
                &accessed_text,
                &format!("TaggedRequest<{class_name}>"),
                &insert_text,
                sort_text,
                replacement_range,
            ));
        }
    }

    // Schema.Error (v4 only)
    if version == EffectMajorVersion::V4 {
        if is_fully_qualified
            || ctx
                .type_parser
                .is_node_reference_to_effect_schema_module_api(data.accessed_object, "Error")
        {
            let schema_type_args = schema_type_args(BRANDED_SCHEMA_CLASS_ERROR);
            let insert_text = if is_fully_qualified {
                format!(
                    r#"{schema_identifier}.Error<{schema_type_args}>("{class_name}")({{${{0}}}}){{}}"#
                )
            } else {
                format!(r#"Error<{schema_type_args}>("{class_name}")({{${{0}}}}){{}}"#)
            };
            items.push(make_extends_completion_item(
                &accessed_text,
                &format!("Error<{class_name}>"),
                &insert_text,
                sort_text,
                replacement_range,
            ));
        }
        if is_fully_qualified
            || ctx
                .type_parser
                .is_node_reference_to_effect_schema_module_api(data.accessed_object, "Opaque")
        {
            let schema_type_args = schema_type_args(BRANDED_SCHEMA_CLASS_OPAQUE);
            let insert_text = if is_fully_qualified {
                format!(r"{schema_identifier}.Opaque<{schema_type_args}>()(${{0}}){{}}")
            } else {
                format!(r"Opaque<{schema_type_args}>()(${{0}}){{}}")
            };
            items.push(make_extends_completion_item(
                &accessed_text,
                &format!("Opaque<{class_name}>"),
                &insert_text,
                sort_text,
                replacement_range,
            ));
        }
    }

    // Model.Class (v4 only)
    if version == EffectMajorVersion::V4 {
        let model_reference = ets_typeparser::find_module_reference_for_package(
            ctx.source_file,
            "effect/schema",
            "Model",
        );
        let model_identifier = model_reference.string();

        let is_model_fully_qualified = model_reference.matches_node(data.accessed_object);

        if is_model_fully_qualified
            || ctx
                .type_parser
                .is_node_reference_to_effect_model_module_api(data.accessed_object, "Class")
        {
            let insert_text = if is_model_fully_qualified {
                format!(r#"{model_identifier}.Class<{class_name}>("{class_name}")({{${{0}}}}){{}}"#)
            } else {
                format!(r#"Class<{class_name}>("{class_name}")({{${{0}}}}){{}}"#)
            };
            items.push(make_extends_completion_item(
                &accessed_text,
                &format!("Class<{class_name}>"),
                &insert_text,
                sort_text,
                replacement_range,
            ));
        }
    }

    items
}

// go: internal/completions/effect_schema_self_in_classes.go:176 makeCompletionItem ea64337443f0ca3b
// makeCompletionItem creates a CompletionItem with snippet format and a text edit.
//
// Rust: Go's variadic `kindOverride ...lsproto.CompletionItemKind` (only its first element is read) is an
// `Option`.
pub(crate) fn make_completion_item(
    label: &str,
    insert_text: &str,
    sort_text: &str,
    replacement_range: lsproto::Range,
    kind_override: Option<lsproto::CompletionItemKind>,
) -> lsproto::CompletionItem {
    let mut kind = lsproto::CompletionItemKind::VARIABLE;
    if let Some(kind_override) = kind_override {
        kind = kind_override;
    }
    let format = lsproto::InsertTextFormat::SNIPPET;
    lsproto::CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        insert_text_format: Some(format),
        sort_text: Some(sort_text.to_string()),
        text_edit: Some(lsproto::TextEditOrInsertReplaceEdit {
            text_edit: Some(lsproto::TextEdit {
                new_text: insert_text.to_string(),
                range: replacement_range,
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}

// go: internal/completions/effect_schema_self_in_classes.go:199 makeExtendsCompletionItem 8038343e7dee1181
// makeExtendsCompletionItem creates a CompletionItem for an extends-clause completion.
// It sets FilterText to accessedText.label so VS Code can filter correctly when the
// replacement range covers the full "Module.Property" expression.
pub(crate) fn make_extends_completion_item(
    accessed_text: &str,
    label: &str,
    insert_text: &str,
    sort_text: &str,
    replacement_range: lsproto::Range,
) -> lsproto::CompletionItem {
    let mut item = make_completion_item(label, insert_text, sort_text, replacement_range, None);
    let filter_text = format!("{accessed_text}.{label}");
    item.filter_text = Some(filter_text);
    item
}

// go: internal/completions/effect_schema_self_in_classes.go:207 byteSpanToRange af9827399dde439e
// byteSpanToRange converts a byte offset span to an LSP Range.
pub(crate) fn byte_span_to_range(ctx: &Context<'_, '_>, start: u32, length: u32) -> lsproto::Range {
    let (start_line, start_char) =
        scanner::get_ecma_line_and_utf16_character_of_position(&*ctx.source_file, start);
    let (end_line, end_char) =
        scanner::get_ecma_line_and_utf16_character_of_position(&*ctx.source_file, start + length);
    lsproto::Range {
        start: lsproto::Position { line: start_line as u32, character: start_char },
        end: lsproto::Position { line: end_line as u32, character: end_char },
    }
}
