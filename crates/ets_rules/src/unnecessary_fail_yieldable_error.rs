use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::This_yield_Asterisk_Effect_fail_passes_a_yieldable_error_value_yield_Asterisk_represents_that_value_directly_without_wrapping_it_in_Effect_fail_effect_unnecessaryFailYieldableError;

// go: internal/rules/unnecessary_fail_yieldable_error.go:16 UnnecessaryFailYieldableError
// UnnecessaryFailYieldableError suggests yielding yieldable errors directly
// instead of wrapping with Effect.fail.
pub static UNNECESSARY_FAIL_YIELDABLE_ERROR: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "unnecessaryFailYieldableError",
    group: "style",
    description: "Suggests yielding yieldable errors directly instead of wrapping with Effect.fail",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_unnecessary_fail_yieldable_error(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/unnecessary_fail_yieldable_error.go:35 UnnecessaryFailYieldableErrorMatch
// UnnecessaryFailYieldableErrorMatch holds the AST nodes needed by both the
// diagnostic rule and the quick-fix for the unnecessaryFailYieldableError pattern.
#[derive(Clone)]
pub struct UnnecessaryFailYieldableErrorMatch {
    pub source_file: P<SourceFile>, // The source file where this match was found
    pub location: TextRange,        // The pre-computed error range for this match
    pub yield_node: P<Node>,        // The yield* expression node
    pub call_node: P<Node>,         // The Effect.fail(...) call expression (fix replaces this)
    pub fail_argument: P<Node>,     // The first argument to Effect.fail (the replacement text)
}

// go: internal/rules/unnecessary_fail_yieldable_error.go:45 AnalyzeUnnecessaryFailYieldableError
// AnalyzeUnnecessaryFailYieldableError finds all yield* Effect.fail(...) calls
// where the argument is a yieldable error type that can be yielded directly.
//
// Rust: Go's ignored checker argument is dropped.
pub fn analyze_unnecessary_fail_yieldable_error(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<UnnecessaryFailYieldableErrorMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<UnnecessaryFailYieldableErrorMatch>,
    ) -> bool {
        if n.kind() == Kind::YieldExpression {
            let yield_ = n.as_yield_expression();
            // Must be yield* (not plain yield)
            if yield_.asterisk_token().is_some()
                && let Some(yield_expression) = yield_.expression()
                && yield_expression.kind() == Kind::CallExpression
            {
                let call = yield_expression.as_call_expression();
                let arguments = call.arguments().nodes();
                if !arguments.is_empty()
                    && tp.is_node_reference_to_effect_module_api(call.expression(), "fail")
                {
                    let arg = arguments[0];
                    let arg_type = tp.get_type_at_location(arg);
                    if let Some(arg_type) = arg_type
                        && tp.is_yieldable_error_type(arg_type)
                    {
                        matches.push(UnnecessaryFailYieldableErrorMatch {
                            source_file: sf,
                            location: ets_shim::scanner::get_error_range_for_node(sf, n),
                            yield_node: n,
                            call_node: yield_expression,
                            fail_argument: arg,
                        });
                    }
                }
            }
        }

        n.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}
