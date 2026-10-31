use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectContextFlags, EffectMajorVersion};

// go: internal/rules/global_json.go:11 GlobalJson 23402d1a88c6c7d8
pub static GLOBAL_JSON: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalJson",
    group: "effectNative",
    description: "Warns when using JSON.parse or JSON.stringify outside Effect generators instead of Effect Schema",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::THIS_CODE_USES_JSON_0_WHICH_THROWS_INSTEAD_OF_FAILING_WITH_A_TYPED_ERROR_1_EFFECT_GLOBAL_JSON.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        run_global_json(
            ctx,
            false,
            &tsdiag::THIS_CODE_USES_JSON_0_WHICH_THROWS_INSTEAD_OF_FAILING_WITH_A_TYPED_ERROR_1_EFFECT_GLOBAL_JSON,
        )
    },
}
});

// go: internal/rules/global_json.go:23 GlobalJsonInEffect 6d48649380ed8fd9
pub static GLOBAL_JSON_IN_EFFECT: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalJsonInEffect",
    group: "effectNative",
    description: "Warns when using JSON.parse or JSON.stringify inside Effect generators instead of Effect Schema",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        tsdiag::THIS_EFFECT_CODE_USES_JSON_0_WHICH_THROWS_INSTEAD_OF_FAILING_WITH_A_TYPED_ERROR_1_EFFECT_GLOBAL_JSON_IN_EFFECT
            .code(),
    ],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        run_global_json(
            ctx,
            true,
            &tsdiag::THIS_EFFECT_CODE_USES_JSON_0_WHICH_THROWS_INSTEAD_OF_FAILING_WITH_A_TYPED_ERROR_1_EFFECT_GLOBAL_JSON_IN_EFFECT,
        )
    },
}
});

// go: internal/rules/global_json.go:35 runGlobalJson 6a99a3124baecdf3
pub(crate) fn run_global_json(
    ctx: &mut Context<'_, '_>,
    check_in_effect: bool,
    message: &'static Message,
) -> Vec<P<Diagnostic>> {
    let recommendation = global_json_recommendation(
        ctx.type_parser.supported_effect_version() == EffectMajorVersion::V4,
    );
    let mut diags = Vec::new();
    for call in find_global_json_calls(ctx, check_in_effect) {
        diags.push(ctx.new_diagnostic(
            ctx.source_file,
            ctx.get_error_range(call.node),
            message,
            &[],
            &[call.method, recommendation],
        ));
    }
    diags
}

// go: internal/rules/global_json.go:44 globalJsonCall 7ca6f42041b2fe86
pub(crate) struct GlobalJsonCall {
    pub(crate) node: P<Node>,
    pub(crate) method: &'static str,
}

// go: internal/rules/global_json.go:51 findGlobalJsonCalls d5df56c0d1577596
// findGlobalJsonCalls returns the calls to the global JSON.parse and
// JSON.stringify whose Effect context matches checkInEffect.
pub(crate) fn find_global_json_calls(
    ctx: &mut Context<'_, '_>,
    check_in_effect: bool,
) -> Vec<GlobalJsonCall> {
    let mut calls = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        ctx: &mut Context<'_, '_>,
        check_in_effect: bool,
        node: P<Node>,
        calls: &mut Vec<GlobalJsonCall>,
    ) -> bool {
        if node.kind() == Kind::CallExpression {
            let in_effect = ctx.type_parser.get_effect_context_flags(node)
                & EffectContextFlags::InEffect
                != EffectContextFlags::None;
            if in_effect == check_in_effect {
                let expression = node.as_call_expression().expression();
                for method in ["parse", "stringify"] {
                    if ctx
                        .type_parser
                        .is_node_reference_to_global_member(expression, "JSON", method)
                    {
                        calls.push(GlobalJsonCall { node, method });
                        break;
                    }
                }
            }
        }

        node.for_each_child(&mut |child| walk(ctx, check_in_effect, child, calls));
        false
    }

    walk(ctx, check_in_effect, ctx.source_file.as_node(), &mut calls);

    calls
}

// go: internal/rules/global_json.go:81 globalJsonRecommendation 50b1a1dfed558733
pub(crate) fn global_json_recommendation(is_v4: bool) -> &'static str {
    if is_v4 {
        return "Use `Schema.fromJsonString(Schema.Unknown)` for unknown shapes, `Schema.fromJsonString(schema)` for known ones, or `Schema.toCodecJson(schema)` when working with JSON values instead of strings.";
    }

    "Use `Schema.parseJson(Schema.Unknown)` for unknown shapes or `Schema.parseJson(schema)` for known ones."
}
