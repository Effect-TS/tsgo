// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{LazyExpressionFlags, TypeParser, parse_lazy_expression};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_OR_ELSE_SUCCEED_EXPRESSES_THE_SAME_RECOVERY_MORE_DIRECTLY_THAN_EFFECT_0_FOLLOWED_BY_EFFECT_SUCCEED_EFFECT_CATCH_TO_OR_ELSE_SUCCEED;

// go: internal/rules/catch_to_or_else_succeed.go:16 CatchToOrElseSucceed 8ea6fffd60474774
// CatchToOrElseSucceed suggests using Effect.orElseSucceed instead of Effect.catch + Effect.succeed.
pub static CATCH_TO_OR_ELSE_SUCCEED: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "catchToOrElseSucceed",
    group: "style",
    description: "Suggests using Effect.orElseSucceed instead of Effect.catch + Effect.succeed",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_catch_to_or_else_succeed(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&m.catch_method_name],
            ));
        }
        diags
    },
});

// go: internal/rules/catch_to_or_else_succeed.go:35 CatchToOrElseSucceedMatch 1bd9bab8d736ff8f
// CatchToOrElseSucceedMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the catchToOrElseSucceed pattern.
#[derive(Clone)]
pub struct CatchToOrElseSucceedMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub callee: P<Node>,
    pub catch_method_name: String,
    pub succeed_call_expression: P<Node>,
    pub succeed_argument: P<Node>,
}

// go: internal/rules/catch_to_or_else_succeed.go:46 AnalyzeCatchToOrElseSucceed 50c2c3b298139a68
// AnalyzeCatchToOrElseSucceed finds Effect.catch callbacks that ignore the error
// and return Effect.succeed, which can be simplified to Effect.orElseSucceed.
//
// Rust: Go's checker argument is the parser's checker; Go ignores it, so it is dropped.
pub fn analyze_catch_to_or_else_succeed(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<CatchToOrElseSucceedMatch> {
    let mut matches = Vec::new();

    let flows = tp.piping_flows(sf, true);
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        for (_, transformation) in flow.transformations().all() {
            if transformation.args().len() < 1 {
                continue;
            }

            let lazy =
                parse_lazy_expression(*transformation.args().at(0), LazyExpressionFlags::Thunk);
            let Some(lazy) = lazy else {
                continue;
            };

            let expr = lazy.expression();
            if expr.kind() != Kind::CallExpression {
                continue;
            }
            let call = expr.as_call_expression();

            // Name the catch variant the callee resolves to, however it is written.
            let catch_method_name = if tp
                .is_node_reference_to_effect_module_api(transformation.callee(), "catch")
            {
                "catch".to_string()
            } else if tp.is_node_reference_to_effect_module_api(transformation.callee(), "catchAll")
            {
                "catchAll".to_string()
            } else {
                continue;
            };

            let arguments = call.arguments().nodes();
            if arguments.is_empty() {
                continue;
            }

            if !tp.is_node_reference_to_effect_module_api(call.expression(), "succeed") {
                continue;
            }

            matches.push(CatchToOrElseSucceedMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
                callee: transformation.callee(),
                catch_method_name,
                succeed_call_expression: expr,
                succeed_argument: arguments[0],
            });
        }
    }

    matches
}
