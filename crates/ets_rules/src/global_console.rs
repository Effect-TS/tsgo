use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::EffectContextFlags;

// go: internal/rules/global_console.go:12 globalConsoleMethodAlternatives b980ad841c4a5976
// Rust: Go's map is iterated in random order; at most one method can match a callee (each names a different member
// of `console`), so the order is not observable and a slice in Go's literal order stands in for it.
pub(crate) static GLOBAL_CONSOLE_METHOD_ALTERNATIVES: &[(&str, &str)] = &[
    ("log", "Effect.log or Logger"),
    ("warn", "Effect.logWarning or Logger"),
    ("error", "Effect.logError or Logger"),
    ("info", "Effect.logInfo or Logger"),
    ("debug", "Effect.logDebug or Logger"),
    ("trace", "Effect.logTrace or Logger"),
];

// go: internal/rules/global_console.go:21 GlobalConsole 5b8ccd1e25e88dd1
pub static GLOBAL_CONSOLE: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalConsole",
    group: "effectNative",
    description: "Warns when using console methods outside Effect generators instead of Effect.log/Logger",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::THIS_CODE_USES_CONSOLE_1_THE_CORRESPONDING_EFFECT_LOGGING_API_IS_0_EFFECT_GLOBAL_CONSOLE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_global_console(ctx, false) },
}
});

// go: internal/rules/global_console.go:33 GlobalConsoleInEffect 693a9969c5bc73a1
pub static GLOBAL_CONSOLE_IN_EFFECT: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalConsoleInEffect",
    group: "effectNative",
    description: "Warns when using console methods inside Effect generators instead of Effect.log/Logger",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::THIS_EFFECT_CODE_USES_CONSOLE_1_LOGGING_IN_EFFECT_CODE_IS_REPRESENTED_THROUGH_0_EFFECT_GLOBAL_CONSOLE_IN_EFFECT.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_global_console(ctx, true) },
}
});

// go: internal/rules/global_console.go:45 runGlobalConsole ee3016e8dcddc76c
pub(crate) fn run_global_console(
    ctx: &mut Context<'_, '_>,
    check_in_effect: bool,
) -> Vec<P<Diagnostic>> {
    let mut message: &'static Message =
        &tsdiag::THIS_CODE_USES_CONSOLE_1_THE_CORRESPONDING_EFFECT_LOGGING_API_IS_0_EFFECT_GLOBAL_CONSOLE;
    if check_in_effect {
        message = &tsdiag::THIS_EFFECT_CODE_USES_CONSOLE_1_LOGGING_IN_EFFECT_CODE_IS_REPRESENTED_THROUGH_0_EFFECT_GLOBAL_CONSOLE_IN_EFFECT;
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
            if in_effect == check_in_effect {
                for &(method, alternative) in GLOBAL_CONSOLE_METHOD_ALTERNATIVES {
                    if !ctx.type_parser.is_node_reference_to_global_member(
                        node.as_call_expression().expression(),
                        "console",
                        method,
                    ) {
                        continue;
                    }
                    diags.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ets_shim::scanner::get_error_range_for_node(ctx.source_file, node),
                        message,
                        &[],
                        &[alternative, method],
                    ));
                    break;
                }
            }
        }

        node.for_each_child(&mut |child| walk(ctx, check_in_effect, message, child, diags));
        false
    }

    walk(ctx, check_in_effect, message, ctx.source_file.as_node(), &mut diags);

    diags
}
