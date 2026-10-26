use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, Symbol, SymbolFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectContextFlags, EffectMajorVersion};

// go: internal/rules/global_fetch.go:12 GlobalFetch
pub static GLOBAL_FETCH: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalFetch",
    group: "effectNative",
    description: "Warns when using the global fetch function outside Effect generators instead of the Effect HTTP client",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::THIS_CODE_USES_THE_GLOBAL_FETCH_FUNCTION_HTTP_REQUESTS_ARE_REPRESENTED_THROUGH_HTTP_CLIENT_FROM_0_EFFECT_GLOBAL_FETCH.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_global_fetch(ctx, false) },
}
});

// go: internal/rules/global_fetch.go:24 GlobalFetchInEffect
pub static GLOBAL_FETCH_IN_EFFECT: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalFetchInEffect",
    group: "effectNative",
    description: "Warns when using the global fetch function inside Effect generators instead of the Effect HTTP client",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::THIS_EFFECT_CODE_CALLS_THE_GLOBAL_FETCH_FUNCTION_HTTP_REQUESTS_IN_EFFECT_CODE_ARE_REPRESENTED_THROUGH_HTTP_CLIENT_FROM_0_EFFECT_GLOBAL_FETCH_IN_EFFECT.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_global_fetch(ctx, true) },
}
});

// go: internal/rules/global_fetch.go:36 runGlobalFetch
pub(crate) fn run_global_fetch(
    ctx: &mut Context<'_, '_>,
    check_in_effect: bool,
) -> Vec<P<Diagnostic>> {
    let Some(fetch_symbol) =
        ctx.checker().resolve_name_exported("fetch", None, SymbolFlags::Value, false)
    else {
        return Vec::new();
    };

    let mut package_name = "effect/http";
    if ctx.type_parser.supported_effect_version() == EffectMajorVersion::V3 {
        package_name = "@effect/platform";
    }

    let mut message: &'static Message = &tsdiag::THIS_CODE_USES_THE_GLOBAL_FETCH_FUNCTION_HTTP_REQUESTS_ARE_REPRESENTED_THROUGH_HTTP_CLIENT_FROM_0_EFFECT_GLOBAL_FETCH;
    if check_in_effect {
        message = &tsdiag::THIS_EFFECT_CODE_CALLS_THE_GLOBAL_FETCH_FUNCTION_HTTP_REQUESTS_IN_EFFECT_CODE_ARE_REPRESENTED_THROUGH_HTTP_CLIENT_FROM_0_EFFECT_GLOBAL_FETCH_IN_EFFECT;
    }

    let mut diags = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function; `env` carries what it captures.
    struct Env {
        check_in_effect: bool,
        fetch_symbol: P<Symbol>,
        package_name: &'static str,
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
                let call = node.as_call_expression();
                let symbol = ctx.type_parser.get_symbol_at_location(call.expression());
                if ctx.type_parser.resolve_to_global_symbol(symbol) == Some(env.fetch_symbol) {
                    diags.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ets_shim::scanner::get_error_range_for_node(
                            ctx.source_file,
                            call.expression(),
                        ),
                        env.message,
                        &[],
                        &[env.package_name],
                    ));
                }
            }
        }

        node.for_each_child(&mut |child| walk(ctx, env, child, diags));
        false
    }

    let env = Env { check_in_effect, fetch_symbol, package_name, message };
    walk(ctx, &env, ctx.source_file.as_node(), &mut diags);

    diags
}
