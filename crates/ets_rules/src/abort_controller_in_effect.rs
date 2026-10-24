use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, Symbol, SymbolFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::EffectContextFlags;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::AbortController_is_manually_constructed_inside_Effect_code_Use_Effect_abortSignal_for_Effect_managed_cancellation_effect_abortControllerInEffect;

// go: internal/rules/abort_controller_in_effect.go:12 AbortControllerInEffect
pub static ABORT_CONTROLLER_IN_EFFECT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "abortControllerInEffect",
    group: "effectNative",
    description: "Warns when manually constructing AbortController inside Effect generators instead of using Effect.abortSignal",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let Some(abort_controller_symbol) =
            ctx.checker().resolve_name_exported("AbortController", None, SymbolFlags::Value, false)
        else {
            return Vec::new();
        };

        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(
            ctx: &mut Context<'_, '_>,
            abort_controller_symbol: P<Symbol>,
            node: P<Node>,
            diags: &mut Vec<P<Diagnostic>>,
        ) -> bool {
            if node.kind() == Kind::NewExpression
                && ctx
                    .type_parser
                    .get_effect_context_flags(node)
                    .intersects(EffectContextFlags::CanYieldEffect)
            {
                let new_expr = node.as_new_expression();
                let symbol = ctx.type_parser.get_symbol_at_location(new_expr.expression());
                if ctx.type_parser.resolve_to_global_symbol(symbol) == Some(abort_controller_symbol)
                {
                    diags.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ets_shim::scanner::get_error_range_for_node(ctx.source_file, node),
                        MESSAGE,
                        &[],
                        &[],
                    ));
                }
            }

            node.for_each_child(&mut |child| walk(ctx, abort_controller_symbol, child, diags));
            false
        }

        let root = ctx.source_file.as_node();
        walk(ctx, abort_controller_symbol, root, &mut diags);
        diags
    },
});
