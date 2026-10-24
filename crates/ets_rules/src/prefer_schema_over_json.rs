// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::scanner::get_text_of_node;
use ets_typeparser::{
    EffectContextFlags, EffectMajorVersion, LazyExpressionFlags, TypeParser, parse_lazy_expression,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::This_code_uses_JSON_parse_or_JSON_stringify_0_effect_preferSchemaOverJson;

// go: internal/rules/prefer_schema_over_json.go:18 PreferSchemaOverJson
// PreferSchemaOverJson detects JSON.parse and JSON.stringify call expressions inside
// Effect contexts (Effect.try or Effect.gen/Effect.fn) and suggests using Effect Schema instead.
// Both Effect v3 and v4 are supported (no version gating), except the simple Effect.try(() => ...)
// thunk form which is V3-only.
pub static PREFER_SCHEMA_OVER_JSON: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "preferSchemaOverJson",
    group: "effectNative",
    description: "Suggests using Effect Schema for JSON operations instead of JSON.parse/JSON.stringify",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();
        let is_v4 = ctx.type_parser.supported_effect_version() == EffectMajorVersion::V4;

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(
            ctx: &mut Context<'_, '_>,
            n: P<Node>,
            is_v4: bool,
            diags: &mut Vec<P<Diagnostic>>,
        ) -> bool {
            if n.kind() == Kind::CallExpression
                && let Some(d) = check_prefer_schema_over_json(ctx, n, is_v4)
            {
                diags.push(d);
            }

            n.for_each_child(&mut |child| walk(ctx, child, is_v4, diags));
            false
        }

        let root = ctx.source_file.as_node();
        walk(ctx, root, is_v4, &mut diags);
        diags
    },
});

// go: internal/rules/prefer_schema_over_json.go:52 checkPreferSchemaOverJson
// checkPreferSchemaOverJson checks a single call expression for JSON.parse/stringify
// inside an Effect context (Effect.try or Effect.gen/Effect.fn).
pub(crate) fn check_prefer_schema_over_json(
    ctx: &mut Context<'_, '_>,
    node: P<Node>,
    is_v4: bool,
) -> Option<P<Diagnostic>> {
    let recommendation = prefer_schema_over_json_recommendation(is_v4);

    // Try each pattern in order
    if let Some(json_node) = check_effect_try_simple(ctx.type_parser, node, is_v4) {
        return Some(ctx.new_diagnostic(
            ctx.source_file,
            ctx.get_error_range(json_node),
            MESSAGE,
            &[],
            &[recommendation],
        ));
    }
    if let Some(json_node) = check_effect_try_object(ctx.type_parser, node) {
        return Some(ctx.new_diagnostic(
            ctx.source_file,
            ctx.get_error_range(json_node),
            MESSAGE,
            &[],
            &[recommendation],
        ));
    }
    if let Some(json_node) = check_json_method_in_effect_gen(ctx.type_parser, node) {
        return Some(ctx.new_diagnostic(
            ctx.source_file,
            ctx.get_error_range(json_node),
            MESSAGE,
            &[],
            &[recommendation],
        ));
    }
    None
}

// go: internal/rules/prefer_schema_over_json.go:68 preferSchemaOverJsonRecommendation
pub(crate) fn prefer_schema_over_json_recommendation(is_v4: bool) -> &'static str {
    if is_v4 {
        return "Use `Schema.UnknownFromJsonString` for unknown shapes, `Schema.fromJsonString(schema)` for known ones, or `Schema.toCodecJson(schema)` when working with JSON values instead of strings.";
    }

    "Use `Schema.parseJson(Schema.Unknown)` for unknown shapes or `Schema.parseJson(schema)` for known ones."
}

// go: internal/rules/prefer_schema_over_json.go:78 parseJsonMethod
// parseJsonMethod checks if a call expression is JSON.parse or JSON.stringify.
// Returns the call expression node if it matches, nil otherwise.
pub(crate) fn parse_json_method(node: Option<P<Node>>) -> Option<P<Node>> {
    let node = node?;
    if node.kind() != Kind::CallExpression {
        return None;
    }

    let call = node.as_call_expression();
    let expr = call.expression();
    if expr.kind() != Kind::PropertyAccessExpression {
        return None;
    }

    let prop = expr.as_property_access_expression();

    // Check that the object is an identifier "JSON"
    let object_expr = prop.expression();
    if object_expr.kind() != Kind::Identifier {
        return None;
    }
    if get_text_of_node(object_expr) != "JSON" {
        return None;
    }

    // Check that the method is "parse" or "stringify"
    let method_name = get_text_of_node(prop.name());
    if method_name != "parse" && method_name != "stringify" {
        return None;
    }

    Some(node)
}

// go: internal/rules/prefer_schema_over_json.go:118 checkEffectTrySimple
// checkEffectTrySimple matches Effect.try(() => JSON.parse/stringify(...)) - simple thunk form.
// This pattern is V3-only (the simple thunk form was removed in V4).
//
// Rust: Go's unused checker parameter is dropped.
pub(crate) fn check_effect_try_simple(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
    is_v4: bool,
) -> Option<P<Node>> {
    if is_v4 {
        return None;
    }

    if node.kind() != Kind::CallExpression {
        return None;
    }
    let call = node.as_call_expression();

    // Must have at least one argument
    let arguments = call.arguments().nodes();
    if arguments.is_empty() {
        return None;
    }

    // Parse the first argument as a lazy expression (thunk=false, like the TS reference)
    let lazy_expr = parse_lazy_expression(arguments[0], LazyExpressionFlags::None)?;

    // Check callee is Effect.try
    if !tp.is_node_reference_to_effect_module_api(call.expression(), "try") {
        return None;
    }

    parse_json_method(Some(lazy_expr.expression()))
}

// go: internal/rules/prefer_schema_over_json.go:148 checkEffectTryObject
// checkEffectTryObject matches Effect.try({ try: () => JSON.parse/stringify(...), ... }) - object form.
//
// Rust: Go's unused checker parameter is dropped.
pub(crate) fn check_effect_try_object(tp: &mut TypeParser<'_>, node: P<Node>) -> Option<P<Node>> {
    if node.kind() != Kind::CallExpression {
        return None;
    }
    let call = node.as_call_expression();

    // Must have at least one argument
    let arguments = call.arguments().nodes();
    if arguments.is_empty() {
        return None;
    }

    // First argument must be an object literal
    let arg = arguments[0];
    if arg.kind() != Kind::ObjectLiteralExpression {
        return None;
    }

    let obj_lit = arg.as_object_literal_expression();

    // Find the "try" property
    let mut try_initializer = None;
    for &prop in obj_lit.properties().nodes() {
        if prop.kind() != Kind::PropertyAssignment {
            continue;
        }
        let pa = prop.as_property_assignment();
        if pa.name().kind() == Kind::Identifier && get_text_of_node(pa.name()) == "try" {
            try_initializer = Some(pa.initializer());
            break;
        }
    }

    let try_initializer = try_initializer?;

    // Parse the try property initializer as a lazy expression
    let lazy_expr = parse_lazy_expression(try_initializer, LazyExpressionFlags::None)?;

    // Check callee is Effect.try
    if !tp.is_node_reference_to_effect_module_api(call.expression(), "try") {
        return None;
    }

    parse_json_method(Some(lazy_expr.expression()))
}

// go: internal/rules/prefer_schema_over_json.go:205 checkJsonMethodInEffectGen
// checkJsonMethodInEffectGen matches direct JSON.parse/stringify inside an Effect generator (Effect.gen or Effect.fn).
//
// Rust: Go's unused checker parameter is dropped.
pub(crate) fn check_json_method_in_effect_gen(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> Option<P<Node>> {
    // First check if this is a JSON method call
    let json_node = parse_json_method(Some(node))?;

    if !tp.get_effect_context_flags(node).intersects(EffectContextFlags::InEffect) {
        return None;
    }

    Some(json_node)
}
