use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, Symbol, SymbolFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::EffectContextFlags;

// go: internal/rules/global_date.go:12 GlobalDate
pub static GLOBAL_DATE: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalDate",
    group: "effectNative",
    description: "Warns when using Date.now() or new Date() outside Effect generators instead of Clock/DateTime",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        tsdiag::THIS_CODE_USES_DATE_NOW_TIME_ACCESS_IS_REPRESENTED_THROUGH_CLOCK_FROM_EFFECT_EFFECT_GLOBAL_DATE.code(),
        tsdiag::THIS_CODE_CONSTRUCTS_NEW_DATE_DATE_VALUES_ARE_REPRESENTED_THROUGH_DATE_TIME_FROM_EFFECT_EFFECT_GLOBAL_DATE.code(),
    ],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_global_date(ctx, false) },
}
});

// go: internal/rules/global_date.go:27 GlobalDateInEffect
pub static GLOBAL_DATE_IN_EFFECT: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "globalDateInEffect",
    group: "effectNative",
    description: "Warns when using Date.now() or new Date() inside Effect generators instead of Clock/DateTime",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        tsdiag::THIS_EFFECT_CODE_USES_DATE_NOW_TIME_ACCESS_IN_EFFECT_CODE_IS_REPRESENTED_THROUGH_CLOCK_FROM_EFFECT_EFFECT_GLOBAL_DATE_IN_EFFECT.code(),
        tsdiag::THIS_EFFECT_CODE_CONSTRUCTS_NEW_DATE_DATE_VALUES_IN_EFFECT_CODE_ARE_REPRESENTED_THROUGH_DATE_TIME_FROM_EFFECT_EFFECT_GLOBAL_DATE_IN_EFFECT.code(),
    ],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_global_date(ctx, true) },
}
});

// go: internal/rules/global_date.go:42 runGlobalDate
pub(crate) fn run_global_date(
    ctx: &mut Context<'_, '_>,
    check_in_effect: bool,
) -> Vec<P<Diagnostic>> {
    let Some(date_symbol) =
        ctx.checker().resolve_name_exported("Date", None, SymbolFlags::Value, false)
    else {
        return Vec::new();
    };

    let mut diags = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        ctx: &mut Context<'_, '_>,
        check_in_effect: bool,
        date_symbol: P<Symbol>,
        node: P<Node>,
        diags: &mut Vec<P<Diagnostic>>,
    ) -> bool {
        if node.kind() != Kind::CallExpression && node.kind() != Kind::NewExpression {
            node.for_each_child(&mut |child| walk(ctx, check_in_effect, date_symbol, child, diags));
            return false;
        }
        let in_effect = ctx.type_parser.get_effect_context_flags(node)
            & EffectContextFlags::InEffect
            != EffectContextFlags::None;
        if in_effect == check_in_effect {
            let mut matched = false;
            let mut message: &'static Message =
                &tsdiag::THIS_CODE_USES_DATE_NOW_TIME_ACCESS_IS_REPRESENTED_THROUGH_CLOCK_FROM_EFFECT_EFFECT_GLOBAL_DATE;

            match node.kind() {
                Kind::CallExpression => {
                    let call = node.as_call_expression();
                    matched = ctx.type_parser.is_node_reference_to_global_member(
                        call.expression(),
                        "Date",
                        "now",
                    );
                    if matched && check_in_effect {
                        message = &tsdiag::THIS_EFFECT_CODE_USES_DATE_NOW_TIME_ACCESS_IN_EFFECT_CODE_IS_REPRESENTED_THROUGH_CLOCK_FROM_EFFECT_EFFECT_GLOBAL_DATE_IN_EFFECT;
                    }
                }
                Kind::NewExpression => {
                    let symbol = ctx
                        .type_parser
                        .get_symbol_at_location(node.as_new_expression().expression());
                    matched = ctx.type_parser.resolve_to_global_symbol(symbol) == Some(date_symbol);
                    message = &tsdiag::THIS_CODE_CONSTRUCTS_NEW_DATE_DATE_VALUES_ARE_REPRESENTED_THROUGH_DATE_TIME_FROM_EFFECT_EFFECT_GLOBAL_DATE;
                    if check_in_effect {
                        message = &tsdiag::THIS_EFFECT_CODE_CONSTRUCTS_NEW_DATE_DATE_VALUES_IN_EFFECT_CODE_ARE_REPRESENTED_THROUGH_DATE_TIME_FROM_EFFECT_EFFECT_GLOBAL_DATE_IN_EFFECT;
                    }
                }
                _ => {}
            }

            if matched {
                diags.push(ctx.new_diagnostic(
                    ctx.source_file,
                    ets_shim::scanner::get_error_range_for_node(ctx.source_file, node),
                    message,
                    &[],
                    &[],
                ));
            }
        }

        node.for_each_child(&mut |child| walk(ctx, check_in_effect, date_symbol, child, diags));
        false
    }

    walk(ctx, check_in_effect, date_symbol, ctx.source_file.as_node(), &mut diags);

    diags
}
