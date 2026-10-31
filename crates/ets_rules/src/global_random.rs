use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::EffectContextFlags;

// go: internal/rules/global_random.go:12 GlobalRandom 195701ee59a9f2c1
pub static GLOBAL_RANDOM: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalRandom",
    group: "effectNative",
    description: "Warns when using Math.random() outside Effect generators instead of the Random service",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::THIS_CODE_USES_MATH_RANDOM_RANDOMNESS_IS_REPRESENTED_THROUGH_THE_EFFECT_RANDOM_SERVICE_EFFECT_GLOBAL_RANDOM.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_global_random(ctx, false) },
}
});

// go: internal/rules/global_random.go:24 GlobalRandomInEffect 40a9dcc9f19f82fd
pub static GLOBAL_RANDOM_IN_EFFECT: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalRandomInEffect",
    group: "effectNative",
    description: "Warns when using Math.random() inside Effect generators instead of the Random service",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::THIS_EFFECT_CODE_USES_MATH_RANDOM_RANDOMNESS_IS_REPRESENTED_THROUGH_THE_EFFECT_RANDOM_SERVICE_EFFECT_GLOBAL_RANDOM_IN_EFFECT.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_global_random(ctx, true) },
}
});

// go: internal/rules/global_random.go:36 runGlobalRandom b9697d1090934340
pub(crate) fn run_global_random(
    ctx: &mut Context<'_, '_>,
    check_in_effect: bool,
) -> Vec<P<Diagnostic>> {
    let mut message: &'static Message = &tsdiag::THIS_CODE_USES_MATH_RANDOM_RANDOMNESS_IS_REPRESENTED_THROUGH_THE_EFFECT_RANDOM_SERVICE_EFFECT_GLOBAL_RANDOM;
    if check_in_effect {
        message = &tsdiag::THIS_EFFECT_CODE_USES_MATH_RANDOM_RANDOMNESS_IS_REPRESENTED_THROUGH_THE_EFFECT_RANDOM_SERVICE_EFFECT_GLOBAL_RANDOM_IN_EFFECT;
    }

    let mut diags = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        ctx: &mut Context<'_, '_>,
        check_in_effect: bool,
        message: &'static Message,
        node: P<Node>,
        diags: &mut Vec<P<Diagnostic>>,
    ) -> bool {
        if node.kind() == Kind::CallExpression {
            let in_effect = ctx.type_parser.get_effect_context_flags(node)
                & EffectContextFlags::InEffect
                != EffectContextFlags::None;
            if in_effect == check_in_effect
                && ctx.type_parser.is_node_reference_to_global_member(
                    node.as_call_expression().expression(),
                    "Math",
                    "random",
                )
            {
                diags.push(ctx.new_diagnostic(
                    ctx.source_file,
                    ets_shim::scanner::get_error_range_for_node(ctx.source_file, node),
                    message,
                    &[],
                    &[],
                ));
            }
        }

        node.for_each_child(&mut |child| walk(ctx, check_in_effect, message, child, diags));
        false
    }

    walk(ctx, check_in_effect, message, ctx.source_file.as_node(), &mut diags);

    diags
}
