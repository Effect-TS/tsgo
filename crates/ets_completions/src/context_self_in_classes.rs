use std::sync::LazyLock;

use ets_completion::{Completion, Context};
use ets_etscore::ResolvedEffectPluginOptions;
use ets_shim::ast::SourceFile;
use ets_shim::checker::Program;
use ets_shim::core::P;
use ets_shim::lsproto;
use ets_typeparser::{EffectMajorVersion, TypeParser};

use crate::*;

// go: internal/completions/context_self_in_classes.go:18 contextSelfInClasses
// contextSelfInClasses provides completion items for Effect context/service class
// constructors when the cursor is in the extends clause of a class declaration.
pub(crate) static CONTEXT_SELF_IN_CLASSES: LazyLock<Completion> = LazyLock::new(|| Completion {
    name: "contextSelfInClasses",
    description: "Provides Context.Tag / Context.Service completions in extends clauses",
    run: run_context_self_in_classes,
});

// go: internal/completions/context_self_in_classes.go:24 runContextSelfInClasses
fn run_context_self_in_classes(ctx: &mut Context<'_, '_>) -> Vec<lsproto::CompletionItem> {
    let Some(data) =
        ets_completion::parse_data_for_extends_class_completion(ctx.source_file, ctx.position)
    else {
        return Vec::new();
    };

    let version = ctx.type_parser.supported_effect_version();
    let class_name = data.class_name_text();
    let replacement_range =
        byte_span_to_range(ctx, data.replacement_start, data.replacement_length);
    let sort_text = "11";
    let tag_key = compute_deterministic_key(
        ctx.program,
        &mut ctx.type_parser,
        ctx.source_file,
        &class_name,
        "service",
    );
    let tp = &mut ctx.type_parser;

    let context_reference = ets_typeparser::find_module_reference(ctx.source_file, "Context");
    let context_identifier = context_reference.string();
    let accessed_text = data.accessed_object_text();
    let is_fully_qualified = context_reference.matches_node(data.accessed_object);

    if version == EffectMajorVersion::V3 {
        if !is_fully_qualified
            && !tp.is_node_reference_to_effect_context_module_api(data.accessed_object, "Tag")
        {
            return Vec::new();
        }

        let insert_text = if is_fully_qualified {
            format!(r#"{context_identifier}.Tag("{tag_key}")<{class_name}, ${{0}}>(){{}}"#)
        } else {
            format!(r#"Tag("{tag_key}")<{class_name}, ${{0}}>(){{}}"#)
        };

        return vec![make_extends_completion_item(
            &accessed_text,
            &format!(r#"Tag("{class_name}")"#),
            &insert_text,
            sort_text,
            replacement_range,
        )];
    }

    if !is_fully_qualified
        && !tp.is_node_reference_to_effect_context_module_api(data.accessed_object, "Service")
    {
        return Vec::new();
    }

    let mut items = Vec::new();
    {
        let insert_text = if is_fully_qualified {
            format!(r#"{context_identifier}.Service<{class_name}, {{${{0}}}}>()("{tag_key}"){{}}"#)
        } else {
            format!(r#"Service<{class_name}, {{${{0}}}}>()("{tag_key}"){{}}"#)
        };
        items.push(make_extends_completion_item(
            &accessed_text,
            &format!("Service<{class_name}, {{}}>"),
            &insert_text,
            sort_text,
            replacement_range,
        ));
    }
    {
        let insert_text = if is_fully_qualified {
            format!(
                r#"{context_identifier}.Service<{class_name}>()("{tag_key}", {{ make: ${{0}} }}){{}}"#
            )
        } else {
            format!(r#"Service<{class_name}>()("{tag_key}", {{ make: ${{0}} }}){{}}"#)
        };
        items.push(make_extends_completion_item(
            &accessed_text,
            &format!("Service<{class_name}>({{ make }})"),
            &insert_text,
            sort_text,
            replacement_range,
        ));
    }

    items
}

// go: internal/completions/context_self_in_classes.go:98 computeDeterministicKey
// computeDeterministicKey computes the key the deterministicKeys rule expects for a class
// of the given target ("service", "error", or "taggedClass").
// Falls back to the class name if keybuilder returns empty.
pub(crate) fn compute_deterministic_key(
    program: &'static dyn Program,
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    class_name: &str,
    target: &str,
) -> String {
    let Some(effect_config) = program.options().effect.get() else {
        return class_name.to_string();
    };
    let resolved_options = ets_pluginoptions::resolve_effect_plugin_options_for_source_file(
        Some(&effect_config),
        sf.file_name(),
        &program.options().config_file_path,
        program.use_case_sensitive_file_names(),
    );
    let key_patterns = ResolvedEffectPluginOptions::get_key_patterns(resolved_options.as_ref());

    let key = ets_keybuilder::create_string_for_source_file(
        program,
        tp,
        sf,
        class_name,
        target,
        key_patterns,
    );
    if key.is_empty() {
        return class_name.to_string();
    }
    key
}
