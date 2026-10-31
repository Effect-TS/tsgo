use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{LazyExpressionFlags, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_SUCCEED_EXPRESSES_THIS_CONSTANT_VALUE_MORE_DIRECTLY_THAN_EFFECT_SYNC_EFFECT_SYNC_TO_SUCCEED;

// go: internal/rules/sync_to_succeed.go:16 SyncToSucceed eaeb502137e818fd
// SyncToSucceed suggests using Effect.succeed when an Effect.sync thunk returns
// a value that is already constant at Effect construction time.
pub static SYNC_TO_SUCCEED: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "syncToSucceed",
    group: "style",
    description: "Suggests using Effect.succeed instead of Effect.sync when the thunk returns a constant value",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_sync_to_succeed(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/sync_to_succeed.go:41 SyncToSucceedMatch 033767cc99b97378
// SyncToSucceedMatch holds the nodes needed by the diagnostic and quick fix.
#[derive(Clone)]
pub struct SyncToSucceedMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub callee: P<Node>,
    pub thunk: P<Node>,
    pub constant_value: P<Node>,
}

// go: internal/rules/sync_to_succeed.go:51 AnalyzeSyncToSucceed 3cd7bf97b3cc2ace
// AnalyzeSyncToSucceed finds Effect.sync thunks whose result is already stable
// when the Effect is constructed.
//
// Rust: Go's ignored checker argument is dropped.
pub fn analyze_sync_to_succeed(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<SyncToSucceedMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<SyncToSucceedMatch>,
    ) -> bool {
        if node.kind() == Kind::CallExpression {
            let call = node.as_call_expression();
            let expression = call.expression();
            let arguments = call.arguments().nodes();
            if arguments.len() == 1
                && tp.is_node_reference_to_effect_module_api(expression, "sync")
                && let Some(lazy) =
                    ets_typeparser::parse_lazy_expression(arguments[0], LazyExpressionFlags::Thunk)
                && tp.is_expression_value_stable_at_location(lazy.expression(), node)
            {
                matches.push(SyncToSucceedMatch {
                    source_file: sf,
                    location: ets_shim::scanner::get_error_range_for_node(sf, expression),
                    callee: expression,
                    thunk: lazy.node(),
                    constant_value: lazy.expression(),
                });
            }
        }

        node.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}
