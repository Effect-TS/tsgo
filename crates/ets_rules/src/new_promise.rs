use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, Symbol, SymbolFlags};
use ets_shim::diagnostics as tsdiag;

// go: internal/rules/new_promise.go:11 NewPromise 6579e055f5c29c51
pub static NEW_PROMISE: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "newPromise",
        group: "effectNative",
        description: "Warns when constructing promises with new Promise instead of using Effect APIs",
        default_severity: Severity::Off,
        supported_effect: vec!["v3", "v4"],
        codes: vec![tsdiag::THIS_CODE_CONSTRUCTS_NEW_PROMISE_PREFER_EFFECT_AP_IS_SUCH_AS_EFFECT_ASYNC_EFFECT_PROMISE_OR_EFFECT_TRY_PROMISE_INSTEAD_OF_MANUAL_PROMISE_CONSTRUCTION_EFFECT_NEW_PROMISE.code()],
        replaced_by: vec![],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            let Some(promise_symbol) =
                ctx.checker().resolve_name_exported("Promise", None, SymbolFlags::Value, false)
            else {
                return Vec::new();
            };

            let mut diags = Vec::new();
            // Rust: Go's recursive `walk` closure is a nested function.
            fn walk(
                ctx: &mut Context<'_, '_>,
                promise_symbol: P<Symbol>,
                node: P<Node>,
                diags: &mut Vec<P<Diagnostic>>,
            ) -> bool {
                if node.kind() == Kind::NewExpression {
                    let new_expr = node.as_new_expression();
                    let symbol = ctx.type_parser.get_symbol_at_location(new_expr.expression);
                    if ctx.type_parser.resolve_to_global_symbol(symbol) == Some(promise_symbol) {
                        diags.push(ctx.new_diagnostic(
                            ctx.source_file,
                            ets_shim::scanner::get_error_range_for_node(ctx.source_file, node),
                            &tsdiag::THIS_CODE_CONSTRUCTS_NEW_PROMISE_PREFER_EFFECT_AP_IS_SUCH_AS_EFFECT_ASYNC_EFFECT_PROMISE_OR_EFFECT_TRY_PROMISE_INSTEAD_OF_MANUAL_PROMISE_CONSTRUCTION_EFFECT_NEW_PROMISE,
                            &[],
                            &[],
                        ));
                    }
                }

                node.for_each_child(&mut |child| walk(ctx, promise_symbol, child, diags));
                false
            }

            let source_file = ctx.source_file;
            walk(ctx, promise_symbol, source_file.as_node(), &mut diags);
            diags
        },
    }
});
