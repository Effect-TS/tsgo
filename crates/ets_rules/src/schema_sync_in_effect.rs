use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectContextFlags, EffectMajorVersion, TypeParser};

/// Rust-only: Go's `map[string]string` of sync method names to their Effect-based names. Go iterates it in random
/// order; every caller stops at the one key that matches a reference (the keys are distinct API names), so the order
/// is not observable and a slice of pairs is used.
pub(crate) type SyncToEffectMethod = &'static [(&'static str, &'static str)];

/// Rust-only: Go's `syncToEffectMethod[methodName]` map lookup.
pub(crate) fn sync_to_effect_method_lookup(
    methods: SyncToEffectMethod,
    name: &str,
) -> &'static str {
    methods
        .iter()
        .find(|(sync_name, _)| *sync_name == name)
        .map_or("", |(_, effect_name)| effect_name)
}

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::X_0_IS_USED_INSIDE_AN_EFFECT_GENERATOR_SCHEMA_1_PRESERVES_THE_TYPED_EFFECT_ERROR_CHANNEL_FOR_THIS_OPERATION_WITHOUT_THROWING_EFFECT_SCHEMA_SYNC_IN_EFFECT;

// go: internal/rules/schema_sync_in_effect.go:14 syncToEffectMethodV3 821c2ba3b4edb93d
// syncToEffectMethodV3 maps Schema sync method names to their Effect-based V3 equivalents.
pub(crate) static SYNC_TO_EFFECT_METHOD_V3: SyncToEffectMethod = &[
    ("decodeSync", "decode"),
    ("decodeUnknownSync", "decodeUnknown"),
    ("encodeSync", "encode"),
    ("encodeUnknownSync", "encodeUnknown"),
];

// go: internal/rules/schema_sync_in_effect.go:22 syncToEffectMethodV4 d765000c593d1bd2
// syncToEffectMethodV4 maps Schema sync method names to their Effect-based V4 equivalents.
pub(crate) static SYNC_TO_EFFECT_METHOD_V4: SyncToEffectMethod = &[
    ("decodeSync", "decodeEffect"),
    ("decodeUnknownSync", "decodeUnknownEffect"),
    ("encodeSync", "encodeEffect"),
    ("encodeUnknownSync", "encodeUnknownEffect"),
];

// go: internal/rules/schema_sync_in_effect.go:31 SchemaSyncInEffect 2a1481214825dad3
// SchemaSyncInEffect detects Schema sync methods (decodeSync, encodeSync, etc.) used inside
// Effect generators and suggests using the Effect-based variants instead.
pub static SCHEMA_SYNC_IN_EFFECT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "schemaSyncInEffect",
    group: "antipattern",
    description: "Suggests using Effect-based Schema methods instead of sync methods inside Effect generators",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let version = ctx.type_parser.supported_effect_version();
        let sync_to_effect_method = if version == EffectMajorVersion::V4 {
            SYNC_TO_EFFECT_METHOD_V4
        } else {
            SYNC_TO_EFFECT_METHOD_V3
        };

        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(
            ctx: &mut Context<'_, '_>,
            n: P<Node>,
            sync_to_effect_method: SyncToEffectMethod,
            diags: &mut Vec<P<Diagnostic>>,
        ) -> bool {
            if n.kind() == Kind::CallExpression
                && let Some(d) = check_schema_sync_in_effect(ctx, n, sync_to_effect_method)
            {
                diags.push(d);
            }

            n.for_each_child(&mut |child| walk(ctx, child, sync_to_effect_method, diags));
            false
        }

        let root = ctx.source_file.as_node();
        walk(ctx, root, sync_to_effect_method, &mut diags);
        diags
    },
});

// go: internal/rules/schema_sync_in_effect.go:71 checkSchemaSyncInEffect adbec8cb47cec7a6
// checkSchemaSyncInEffect checks a single call expression for Schema sync methods inside an Effect generator.
pub(crate) fn check_schema_sync_in_effect(
    ctx: &mut Context<'_, '_>,
    node: P<Node>,
    sync_to_effect_method: SyncToEffectMethod,
) -> Option<P<Diagnostic>> {
    if node.kind() != Kind::CallExpression {
        return None;
    }
    let call = node.as_call_expression();

    let callee = call.expression();

    // Check if the callee is one of the Schema sync methods (try both ParseResult and SchemaParser modules)
    let method_name = match_schema_sync_method(ctx.type_parser, callee, sync_to_effect_method);
    if method_name.is_empty() {
        return None;
    }

    if !ctx.type_parser.get_effect_context_flags(node).intersects(EffectContextFlags::InEffect) {
        return None;
    }

    let callee_text =
        ets_shim::scanner::get_source_text_of_node_from_source_file(ctx.source_file, callee, false);
    let effect_method_name = sync_to_effect_method_lookup(sync_to_effect_method, method_name);

    Some(ctx.new_diagnostic(
        ctx.source_file,
        ctx.get_error_range(callee),
        MESSAGE,
        &[],
        &[&callee_text, effect_method_name],
    ))
}

// go: internal/rules/schema_sync_in_effect.go:97 matchSchemaSyncMethod 66a4dab5068a27f9
// matchSchemaSyncMethod checks if the node references one of the Schema sync methods via
// either the ParseResult module (V3) or the SchemaParser module (V4).
//
// Rust: Go's ignored checker argument is dropped.
pub(crate) fn match_schema_sync_method(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
    sync_to_effect_method: SyncToEffectMethod,
) -> &'static str {
    for &(method_name, _) in sync_to_effect_method {
        if tp.is_node_reference_to_effect_parse_result_module_api(node, method_name) {
            return method_name;
        }
        if tp.is_node_reference_to_effect_schema_parser_module_api(node, method_name) {
            return method_name;
        }
    }
    ""
}
