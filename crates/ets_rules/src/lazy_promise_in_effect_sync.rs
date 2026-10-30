use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::Diagnostic;
use ets_shim::checker::{SignatureKind, Type};
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{PipingFlow, TypeParser};

// go: internal/rules/lazy_promise_in_effect_sync.go:13 LazyPromiseInEffectSync 35ce48fbf604e822
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
        let flows = ctx.type_parser.piping_flows(ctx.source_file, false);
        for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
            for (index, _) in flow.transformations().all() {
                // Effect.sync(thunk) and the point-free pipe(thunk, Effect.sync)
                // both apply Effect.sync to the step input without extra arguments.
                let transformation = flow.transformations().at(index);
                if transformation.args().len() != 0
                    || !ctx.type_parser.is_node_reference_to_effect_module_api(transformation.callee(), "sync")
                {
                    continue;
                }
                let lazy_arg_type = PipingFlow::transformation_input_type(Some(flow), ctx.type_parser, index);
                let Some(lazy_arg_type) = lazy_arg_type else {
                    continue;
                };
                if !thunk_returns_promise(ctx.type_parser, lazy_arg_type) {
                    continue;
                }
                let location = PipingFlow::transformation_input_node(Some(flow), index)
                    .unwrap_or_else(|| transformation.callee());
                diags.push(ctx.new_diagnostic(
                    ctx.source_file,
                    ets_shim::scanner::get_error_range_for_node(ctx.source_file, location),
                    &tsdiag::THIS_EFFECT_SYNC_THUNK_RETURNS_A_PROMISE_USE_EFFECT_PROMISE_OR_EFFECT_TRY_PROMISE_TO_REPRESENT_ASYNC_WORK_EFFECT_LAZY_PROMISE_IN_EFFECT_SYNC,
                    &[],
                    &[],
                ));
            }
        }
        diags
    },
}
});

// go: internal/rules/lazy_promise_in_effect_sync.go:52 thunkReturnsPromise b09f1edf3ea700a2
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
