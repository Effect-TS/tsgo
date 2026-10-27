use std::sync::LazyLock;

use ets_completion::{Completion, Context};
use ets_shim::lsproto;
use ets_typeparser::EffectMajorVersion;

use crate::*;

// go: internal/completions/effect_rpc_make_classes.go:14 rpcMakeClasses f45edb582e4dc71f
// rpcMakeClasses provides completion items for @effect/rpc Rpc.make
// when the cursor is in the extends clause of a class declaration.
// This is a V3-only completion.
pub(crate) static RPC_MAKE_CLASSES: LazyLock<Completion> = LazyLock::new(|| Completion {
    name: "rpcMakeClasses",
    description: "Provides @effect/rpc Rpc.make completions in extends clauses",
    run: run_rpc_make_classes,
});

// go: internal/completions/effect_rpc_make_classes.go:20 runRpcMakeClasses 6290849567031e03
fn run_rpc_make_classes(ctx: &mut Context<'_, '_>) -> Vec<lsproto::CompletionItem> {
    let Some(data) =
        ets_completion::parse_data_for_extends_class_completion(ctx.source_file, ctx.position)
    else {
        return Vec::new();
    };

    let tp = &mut ctx.type_parser;

    // V3 only
    let version = tp.supported_effect_version();
    if version != EffectMajorVersion::V3 {
        return Vec::new();
    }

    let rpc_reference =
        ets_typeparser::find_module_reference_for_package(ctx.source_file, "@effect/rpc", "Rpc");
    let rpc_identifier = rpc_reference.string();
    let accessed_text = data.accessed_object_text();

    // Only fully-qualified case (e.g., Rpc.make)
    if !rpc_reference.matches_node(data.accessed_object) {
        return Vec::new();
    }

    let class_name = data.class_name_text();

    // Build replacement range from byte offsets
    let replacement_range =
        byte_span_to_range(ctx, data.replacement_start, data.replacement_length);

    let sort_text = "11";
    let insert_text = format!(r#"{rpc_identifier}.make("{class_name}", {{${{0}}}}) {{}}"#);

    vec![make_extends_completion_item(
        &accessed_text,
        &format!(r#"make("{class_name}")"#),
        &insert_text,
        sort_text,
        replacement_range,
    )]
}
