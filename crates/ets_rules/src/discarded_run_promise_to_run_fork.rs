use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, PipingFlowTransformation, TransformationKind, TypeParser,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::THE_PROMISE_RETURNED_BY_0_IS_DISCARDED_SO_ANY_FAILURE_OR_DEFECT_BECOMES_AN_UNHANDLED_REJECTION_USE_1_TO_RUN_IT_FIRE_AND_FORGET_AS_A_FIBER_EFFECT_DISCARDED_RUN_PROMISE_TO_RUN_FORK;

// go: internal/rules/discarded_run_promise_to_run_fork.go:19 DiscardedRunPromiseToRunFork c4c7854c3556186e
// DiscardedRunPromiseToRunFork reports runPromise runs whose Promise is
// discarded: any failure or defect of the effect then becomes an unhandled
// rejection, while runFork states the fire-and-forget intent with a Fiber.
pub static DISCARDED_RUN_PROMISE_TO_RUN_FORK: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "discardedRunPromiseToRunFork",
    group: "correctness",
    description: "Detects discarded Promises from Effect runPromise runners that should use runFork",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_discarded_run_promise_to_run_fork(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.runner_name, m.replacement_name],
            ));
        }
        diagnostics
    },
});

// go: internal/rules/discarded_run_promise_to_run_fork.go:48 DiscardedRunPromiseToRunForkMatch af2d38ca523df43e
// DiscardedRunPromiseToRunForkMatch holds what the diagnostic and quick fix
// need. RenameNode is the runner's property name to rename to RenameTo, or nil
// when the fix is withheld.
#[derive(Clone)]
pub struct DiscardedRunPromiseToRunForkMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub runner_name: &'static str,
    pub replacement_name: &'static str,
    pub rename_node: Option<P<Node>>,
    pub rename_to: &'static str,
}

// go: internal/rules/discarded_run_promise_to_run_fork.go:61 AnalyzeDiscardedRunPromiseToRunFork e180937bd98d9ac3
// AnalyzeDiscardedRunPromiseToRunFork finds piping flows ending in a
// runPromise run whose resulting Promise is discarded, in any spelling of the
// step. A runner that is not the flow's last step is consumed by the next
// step.
//
// Rust: Go's checker argument is the parser's checker; Go ignores it, so it is dropped. A transformation's
// callee is never nil here, so Go's `last.Callee() == nil` check drops.
pub fn analyze_discarded_run_promise_to_run_fork(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<DiscardedRunPromiseToRunForkMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, false);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        let transformations = flow.transformations();
        if transformations.len() == 0 {
            continue;
        }
        let last = transformations.at(transformations.len() - 1);
        if !is_discarded_expression(Some(flow.node())) {
            continue;
        }
        let Some((mut m, location_node)) = discarded_run_promise_runner(tp, sf, flow.node(), last)
        else {
            continue;
        };
        if m.rename_node.is_some()
            && last.kind() == TransformationKind::CallWithOptionals
            && tp.supported_effect_version() == EffectMajorVersion::V3
        {
            // Effect v3 runFork options do not accept runPromise's signal.
            m.rename_node = None;
        }
        m.source_file = sf;
        m.location = ets_shim::scanner::get_error_range_for_node(sf, location_node);
        matches.push(m);
    }

    ets_go::sort::slice(&mut matches, |a, b| a.location.pos() < b.location.pos());
    matches
}

// go: internal/rules/discarded_run_promise_to_run_fork.go:98 discardedRunner c863d7bad21741b2
// discardedRunner describes a recognized Promise runner and the Fiber runner
// its fix renames it to.
pub(crate) struct DiscardedRunner {
    name: &'static str,
    replacement: &'static str,
    rename_to: &'static str,
    // ownsReplacement reports whether the runner's receiver is known to expose
    // the replacement with its declared signature, so the rename is sound.
    owns_replacement: fn(tp: &mut TypeParser<'_>, receiver: P<Node>) -> bool,
}

// go: internal/rules/discarded_run_promise_to_run_fork.go:107 effectRunPromiseRunner 3b8e39489ee3be6e
pub(crate) static EFFECT_RUN_PROMISE_RUNNER: DiscardedRunner = DiscardedRunner {
    name: "Effect.runPromise",
    replacement: "Effect.runFork",
    rename_to: "runFork",
    owns_replacement: |tp, receiver| tp.is_expression_effect_module(receiver),
};
// go: internal/rules/discarded_run_promise_to_run_fork.go:107 effectRunPromiseRunner 3b8e39489ee3be6e
pub(crate) static EFFECT_RUN_PROMISE_WITH_RUNNER: DiscardedRunner = DiscardedRunner {
    name: "Effect.runPromiseWith",
    replacement: "Effect.runForkWith",
    rename_to: "runForkWith",
    owns_replacement: |tp, receiver| tp.is_expression_effect_module(receiver),
};
// go: internal/rules/discarded_run_promise_to_run_fork.go:107 effectRunPromiseRunner 3b8e39489ee3be6e
pub(crate) static RUNTIME_RUN_PROMISE_RUNNER: DiscardedRunner = DiscardedRunner {
    name: "Runtime.runPromise",
    replacement: "Runtime.runFork",
    rename_to: "runFork",
    owns_replacement: |tp, receiver| tp.is_expression_effect_runtime_module(receiver),
};
// go: internal/rules/discarded_run_promise_to_run_fork.go:107 effectRunPromiseRunner 3b8e39489ee3be6e
pub(crate) static MANAGED_RUNTIME_RUN_PROMISE_RUNNER: DiscardedRunner = DiscardedRunner {
    name: "runPromise",
    replacement: "runFork",
    rename_to: "runFork",
    owns_replacement: |tp, receiver| tp.is_expression_plain_effect_managed_runtime(receiver),
};

// go: internal/rules/discarded_run_promise_to_run_fork.go:128 discardedRunPromiseRunner 7153c70b492ae392
// discardedRunPromiseRunner classifies the runner of the flow's last step and
// returns the node the diagnostic is reported on.
//
// Rust: Go's `(match, node, ok)` is an `Option`. The match is built with `sf`, which the caller sets on it
// anyway (Go leaves the field nil until then); Go's `newMatch` closure is the nested `new_match`.
pub(crate) fn discarded_run_promise_runner(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    flow_node: P<Node>,
    transformation: &PipingFlowTransformation,
) -> Option<(DiscardedRunPromiseToRunForkMatch, P<Node>)> {
    let callee = ast::skip_parentheses(transformation.callee());
    fn new_match(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        runner_node: P<Node>,
        runner: &DiscardedRunner,
        renameable: bool,
    ) -> (DiscardedRunPromiseToRunForkMatch, P<Node>) {
        let mut m = DiscardedRunPromiseToRunForkMatch {
            source_file: sf,
            location: TextRange::default(),
            runner_name: runner.name,
            replacement_name: runner.replacement,
            rename_node: None,
            rename_to: runner.rename_to,
        };
        // Renaming in place is only sound on the receiver that owns both
        // runners: the Effect or Runtime module itself, or a plain
        // ManagedRuntime. A narrowed or synthesized receiver may lack the
        // replacement or change its type, so it keeps only the diagnostic.
        if renameable && runner_node.kind() == Kind::PropertyAccessExpression {
            let access = runner_node.as_property_access_expression();
            if (runner.owns_replacement)(tp, ast::skip_parentheses(access.expression())) {
                m.rename_node = Some(access.name());
            }
        }
        (m, runner_node)
    }

    if tp.is_node_reference_to_effect_module_api(callee, "runPromise") {
        return Some(new_match(tp, sf, callee, &EFFECT_RUN_PROMISE_RUNNER, true));
    } else if tp.is_node_reference_to_effect_managed_runtime_member(callee, "runPromise") {
        return Some(new_match(tp, sf, callee, &MANAGED_RUNTIME_RUN_PROMISE_RUNNER, true));
    }

    // The factory with its context or runtime as the step's argument, as in
    // effect.pipe(Effect.runPromiseWith(context)) or the data-first
    // Runtime.runPromise(runtime, effect). Without that argument the step only
    // builds the runner.
    if transformation.args().len() == 1
        && let Some(runner) = discarded_run_promise_factory(tp, callee)
    {
        return Some(new_match(tp, sf, callee, runner, true));
    }

    // Curried: flows keep the factory call as the step's callee, as in
    // Effect.runPromiseWith(context)(effect).
    if callee.kind() == Kind::CallExpression {
        let factory = callee.as_call_expression();
        if factory.arguments().nodes().len() == 1 {
            let runner_node = ast::skip_parentheses(factory.expression());
            if let Some(runner) = discarded_run_promise_factory(tp, runner_node) {
                return Some(new_match(tp, sf, runner_node, runner, true));
            }
        }
    }

    // Hoisted: the callee is a binding of the factory call's result, so there
    // is no runner name to rename in place. Flows do not expose the call of a
    // pipe or pipeable step, so only the applied spellings, whose call is the
    // flow node, are resolved.
    let kind = transformation.kind();
    if kind != TransformationKind::Call && kind != TransformationKind::CallWithOptionals {
        return None;
    }
    if let Some(owner) = tp.returned_function_call_owner(flow_node)
        && let Some(runner) = discarded_run_promise_factory(tp, owner)
    {
        return Some(new_match(tp, sf, callee, runner, false));
    }
    None
}

// go: internal/rules/discarded_run_promise_to_run_fork.go:195 discardedRunPromiseFactory 41ab489118816681
// discardedRunPromiseFactory recognizes the runners that take a context or
// runtime first and return the Promise runner.
pub(crate) fn discarded_run_promise_factory(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> Option<&'static DiscardedRunner> {
    if tp.is_node_reference_to_effect_module_api(node, "runPromiseWith") {
        return Some(&EFFECT_RUN_PROMISE_WITH_RUNNER);
    } else if tp.is_node_reference_to_effect_runtime_module_api(node, "runPromise") {
        return Some(&RUNTIME_RUN_PROMISE_RUNNER);
    }
    None
}

// go: internal/rules/discarded_run_promise_to_run_fork.go:208 isDiscardedExpression b72a38d38e43d282
// isDiscardedExpression reports whether the value of node is discarded: it is
// an expression statement, the operand of void, or a comma operand whose value
// is not used. Parentheses are transparent.
pub(crate) fn is_discarded_expression(node: Option<P<Node>>) -> bool {
    let mut node = node;
    while let Some(current) = node
        && let Some(current_parent) = current.parent()
    {
        let Some(parent) = ast::walk_up_parenthesized_expressions(current_parent) else {
            return false;
        };
        match parent.kind() {
            Kind::ExpressionStatement | Kind::VoidExpression => return true,
            Kind::BinaryExpression => {
                let binary = parent.as_binary_expression();
                if binary.operator_token().kind() != Kind::CommaToken {
                    return false;
                }
                if ast::skip_parentheses(binary.left()) == current {
                    return true;
                }
                node = Some(parent);
                continue;
            }
            _ => {}
        }
        return false;
    }
    false
}
