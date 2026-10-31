use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SymbolFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::EffectContextFlags;

/// Rust-only: the rule's message (Go spells it out at each use).
static PROCESS_ENV_MESSAGE: &Message =
    &tsdiag::THIS_CODE_READS_FROM_PROCESS_ENV_ENVIRONMENT_CONFIGURATION_IS_REPRESENTED_THROUGH_CONFIG_FROM_EFFECT_EFFECT_PROCESS_ENV;

/// Rust-only: the rule's message (Go spells it out at each use).
static PROCESS_ENV_IN_EFFECT_MESSAGE: &Message =
    &tsdiag::THIS_EFFECT_CODE_READS_FROM_PROCESS_ENV_ENVIRONMENT_CONFIGURATION_IN_EFFECT_CODE_IS_REPRESENTED_THROUGH_CONFIG_FROM_EFFECT_EFFECT_PROCESS_ENV_IN_EFFECT;

// go: internal/rules/process_env.go:12 ProcessEnv 9ff67b597e05b208
pub static PROCESS_ENV: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "processEnv",
    group: "effectNative",
    description: "Warns when reading process.env outside Effect generators instead of using Effect Config",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![PROCESS_ENV_MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_process_env(ctx, false) },
});

// go: internal/rules/process_env.go:24 ProcessEnvInEffect 75a1746bf0fa03fc
pub static PROCESS_ENV_IN_EFFECT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "processEnvInEffect",
    group: "effectNative",
    description: "Warns when reading process.env inside Effect generators instead of using Effect Config",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![PROCESS_ENV_IN_EFFECT_MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_process_env(ctx, true) },
});

// go: internal/rules/process_env.go:36 runProcessEnv f46b08818c2f1531
pub(crate) fn run_process_env(
    ctx: &mut Context<'_, '_>,
    check_in_effect: bool,
) -> Vec<P<Diagnostic>> {
    let Some(process_symbol) =
        ctx.checker().resolve_name_exported("process", None, SymbolFlags::Value, false)
    else {
        return Vec::new();
    };

    let mut message: &'static Message = PROCESS_ENV_MESSAGE;
    if check_in_effect {
        message = PROCESS_ENV_IN_EFFECT_MESSAGE;
    }

    let mut diags = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        ctx: &mut Context<'_, '_>,
        node: P<Node>,
        check_in_effect: bool,
        process_symbol: P<ets_shim::ast::Symbol>,
        message: &'static Message,
        diags: &mut Vec<P<Diagnostic>>,
    ) -> bool {
        if let Some(process_node) = process_env_root(Some(node)) {
            let in_effect = ctx
                .type_parser
                .get_effect_context_flags(node)
                .intersects(EffectContextFlags::InEffect);
            if in_effect == check_in_effect {
                let symbol = ctx.type_parser.get_symbol_at_location(process_node);
                if ctx.type_parser.resolve_to_global_symbol(symbol) == Some(process_symbol) {
                    diags.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ets_shim::scanner::get_error_range_for_node(ctx.source_file, node),
                        message,
                        &[],
                        &[],
                    ));
                }
            }
        }

        node.for_each_child(&mut |child| {
            walk(ctx, child, check_in_effect, process_symbol, message, diags)
        });
        false
    }

    let root = ctx.source_file.as_node();
    walk(ctx, root, check_in_effect, process_symbol, message, &mut diags);

    diags
}

// go: internal/rules/process_env.go:78 processEnvRoot 0b15de75dc119e11
pub(crate) fn process_env_root(node: Option<P<Node>>) -> Option<P<Node>> {
    let node = node?;
    if node.kind() != Kind::PropertyAccessExpression && node.kind() != Kind::ElementAccessExpression
    {
        return None;
    }
    let access = node.expression();
    if !is_env_property_access(access) {
        return None;
    }
    access?.expression()
}

// go: internal/rules/process_env.go:92 isEnvPropertyAccess 19ceeff17045b993
pub(crate) fn is_env_property_access(node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if node.kind() != Kind::PropertyAccessExpression {
        return false;
    }
    node.as_property_access_expression().name().text() == "env"
}
