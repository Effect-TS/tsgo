use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, Symbol, SymbolFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::EffectContextFlags;

// go: internal/rules/global_timers.go:12 globalTimerAlternative
#[derive(Clone, Copy)]
pub(crate) struct GlobalTimerAlternative {
    pub(crate) name: &'static str,
    pub(crate) alternative: &'static str,
}

// go: internal/rules/global_timers.go:17 globalTimerAlternatives
// Rust: Go's map is iterated in random order; each key resolves to a different global symbol, so at most one entry
// matches a callee and the order is not observable. A slice in Go's literal order stands in for the map.
pub(crate) static GLOBAL_TIMER_ALTERNATIVES: &[(&str, GlobalTimerAlternative)] = &[
    (
        "setTimeout",
        GlobalTimerAlternative { name: "setTimeout", alternative: "Effect.sleep or Schedule" },
    ),
    (
        "setInterval",
        GlobalTimerAlternative { name: "setInterval", alternative: "Schedule or Effect.repeat" },
    ),
];

// go: internal/rules/global_timers.go:28 GlobalTimers
pub static GLOBAL_TIMERS: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalTimers",
    group: "effectNative",
    description: "Warns when using setTimeout/setInterval outside Effect generators instead of Effect.sleep/Schedule",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::This_code_uses_1_the_corresponding_Effect_timer_API_is_0_from_Effect_effect_globalTimers.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_global_timers(ctx, false) },
}
});

// go: internal/rules/global_timers.go:40 GlobalTimersInEffect
pub static GLOBAL_TIMERS_IN_EFFECT: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalTimersInEffect",
    group: "effectNative",
    description: "Warns when using setTimeout/setInterval inside Effect generators instead of Effect.sleep/Schedule",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::This_Effect_code_uses_1_the_corresponding_timer_API_in_this_context_is_0_from_Effect_effect_globalTimersInEffect.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_global_timers(ctx, true) },
}
});

// go: internal/rules/global_timers.go:52 runGlobalTimers
pub(crate) fn run_global_timers(
    ctx: &mut Context<'_, '_>,
    check_in_effect: bool,
) -> Vec<P<Diagnostic>> {
    // Rust: Go's `map[string]*ast.Symbol` as a vector of (name, symbol) pairs (see `GLOBAL_TIMER_ALTERNATIVES`).
    let mut global_symbols: Vec<(&'static str, P<Symbol>)> =
        Vec::with_capacity(GLOBAL_TIMER_ALTERNATIVES.len());
    for &(name, _) in GLOBAL_TIMER_ALTERNATIVES {
        if let Some(symbol) =
            ctx.checker().resolve_name_exported(name, None, SymbolFlags::Value, false)
        {
            global_symbols.push((name, symbol));
        }
    }
    if global_symbols.is_empty() {
        return Vec::new();
    }

    let mut message: &'static Message =
        &tsdiag::This_code_uses_1_the_corresponding_Effect_timer_API_is_0_from_Effect_effect_globalTimers;
    if check_in_effect {
        message = &tsdiag::This_Effect_code_uses_1_the_corresponding_timer_API_in_this_context_is_0_from_Effect_effect_globalTimersInEffect;
    }

    let mut diags = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function; `env` carries what it captures.
    struct Env {
        check_in_effect: bool,
        global_symbols: Vec<(&'static str, P<Symbol>)>,
        message: &'static Message,
    }
    fn walk(
        ctx: &mut Context<'_, '_>,
        env: &Env,
        node: P<Node>,
        diags: &mut Vec<P<Diagnostic>>,
    ) -> bool {
        if node.kind() == Kind::CallExpression {
            let in_effect = ctx.type_parser.get_effect_context_flags(node)
                & EffectContextFlags::InEffect
                != EffectContextFlags::None;
            if in_effect == env.check_in_effect {
                let symbol =
                    ctx.type_parser.get_symbol_at_location(node.as_call_expression().expression());
                let resolved = ctx.type_parser.resolve_to_global_symbol(symbol);
                if let Some(resolved) = resolved {
                    for &(name, global_symbol) in &env.global_symbols {
                        if resolved != global_symbol {
                            continue;
                        }
                        let alt = GLOBAL_TIMER_ALTERNATIVES
                            .iter()
                            .find(|(key, _)| *key == name)
                            .map(|(_, alt)| *alt)
                            .expect("every global symbol has an alternative");
                        diags.push(ctx.new_diagnostic(
                            ctx.source_file,
                            ets_shim::scanner::get_error_range_for_node(ctx.source_file, node),
                            env.message,
                            &[],
                            &[alt.alternative, alt.name],
                        ));
                        break;
                    }
                }
            }
        }

        node.for_each_child(&mut |child| walk(ctx, env, child, diags));
        false
    }

    let env = Env { check_in_effect, global_symbols, message };
    walk(ctx, &env, ctx.source_file.as_node(), &mut diags);

    diags
}
