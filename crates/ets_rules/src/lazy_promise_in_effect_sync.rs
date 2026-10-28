use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::checker::{SignatureKind, Type};
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::TypeParser;

// go: internal/rules/lazy_promise_in_effect_sync.go:13 LazyPromiseInEffectSync e9f564e8a4def0e1
pub static LAZY_PROMISE_IN_EFFECT_SYNC: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "lazyPromiseInEffectSync",
    group: "antipattern",
    description: "Warns when Effect.sync lazily returns a Promise instead of using an async Effect constructor",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        tsdiag::THIS_EFFECT_SYNC_THUNK_RETURNS_A_PROMISE_USE_EFFECT_PROMISE_OR_EFFECT_TRY_PROMISE_TO_REPRESENT_ASYNC_WORK_EFFECT_LAZY_PROMISE_IN_EFFECT_SYNC.code(),
    ],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(ctx: &mut Context<'_, '_>, node: P<Node>, diags: &mut Vec<P<Diagnostic>>) -> bool {
            if node.kind() == Kind::CallExpression {
                let call = node.as_call_expression();
                let args = call.arguments().nodes();
                if !args.is_empty()
                    && ctx.type_parser.is_node_reference_to_effect_module_api(call.expression(), "sync")
                {
                    let lazy_arg = args[0];
                    let lazy_arg_type = ctx.type_parser.get_type_at_location(lazy_arg);
                    if let Some(lazy_arg_type) = lazy_arg_type
                        && thunk_returns_promise(ctx.type_parser, lazy_arg_type)
                    {
                        diags.push(ctx.new_diagnostic(
                            ctx.source_file,
                            ets_shim::scanner::get_error_range_for_node(ctx.source_file, lazy_arg),
                            &tsdiag::THIS_EFFECT_SYNC_THUNK_RETURNS_A_PROMISE_USE_EFFECT_PROMISE_OR_EFFECT_TRY_PROMISE_TO_REPRESENT_ASYNC_WORK_EFFECT_LAZY_PROMISE_IN_EFFECT_SYNC,
                            &[],
                            &[],
                        ));
                    }
                }
            }

            node.for_each_child(&mut |child| walk(ctx, child, diags));
            false
        }

        walk(ctx, ctx.source_file.as_node(), &mut diags);
        diags
    },
}
});

// go: internal/rules/lazy_promise_in_effect_sync.go:55 thunkReturnsPromise b09f1edf3ea700a2
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn thunk_returns_promise(tp: &mut TypeParser<'_>, lazy_arg_type: P<Type>) -> bool {
    for member in tp.unroll_union_members(lazy_arg_type).unwrap_or_default() {
        for &signature in tp.checker().get_signatures_of_type(member, SignatureKind::Call) {
            let return_type = tp.checker().get_return_type_of_signature(signature);
            if tp.promise_type(return_type).is_some() {
                return true;
            }
        }
    }
    false
}
