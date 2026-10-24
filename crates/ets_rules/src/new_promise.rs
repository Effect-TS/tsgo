use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, Symbol, SymbolFlags};
use ets_shim::diagnostics as tsdiag;

// go: internal/rules/new_promise.go:11 NewPromise
pub static NEW_PROMISE: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "newPromise",
        group: "effectNative",
        description: "Warns when constructing promises with new Promise instead of using Effect APIs",
        default_severity: Severity::Off,
        supported_effect: vec!["v3", "v4"],
        codes: vec![tsdiag::This_code_constructs_new_Promise_prefer_Effect_APIs_such_as_Effect_async_Effect_promise_or_Effect_tryPromise_instead_of_manual_Promise_construction_effect_newPromise.code()],
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
                            &tsdiag::This_code_constructs_new_Promise_prefer_Effect_APIs_such_as_Effect_async_Effect_promise_or_Effect_tryPromise_instead_of_manual_Promise_construction_effect_newPromise,
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
