// Package rules contains all Effect diagnostic rule implementations.

use std::sync::{Arc, LazyLock};

use ets_etscore::Severity;
use ets_readonly::Slice;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::checker::SignatureKind;
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{ParsedPipeCallResult, TransformationKind, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_EXPRESSION_CONTAINS_CHAINED_PIPE_CALLS_THAT_CAN_BE_SIMPLIFIED_TO_A_SINGLE_PIPE_CALL_EFFECT_UNNECESSARY_PIPE_CHAIN;

// go: internal/rules/unnecessary_pipe_chain.go:18 UnnecessaryPipeChain
// UnnecessaryPipeChain detects chained pipe() and .pipe() calls that can
// be simplified to a single pipe call.
pub static UNNECESSARY_PIPE_CHAIN: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "unnecessaryPipeChain",
    group: "style",
    description: "Simplifies chained pipe calls into a single pipe call",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_unnecessary_pipe_chain(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/unnecessary_pipe_chain.go:37 UnnecessaryPipeChainMatch
// UnnecessaryPipeChainMatch holds the diagnostic and parsed pipe call results
// needed by both the diagnostic rule and the quick-fix.
#[derive(Clone)]
pub struct UnnecessaryPipeChainMatch {
    pub source_file: P<SourceFile>, // The source file where the diagnostic should be reported
    pub location: TextRange,        // The pre-computed error range for this match
    pub outer: Arc<ParsedPipeCallResult>, // The outer pipe call parse result
    pub inner: Arc<ParsedPipeCallResult>, // The inner pipe call parse result (subject of outer)
}

// go: internal/rules/unnecessary_pipe_chain.go:47 AnalyzeUnnecessaryPipeChain
// AnalyzeUnnecessaryPipeChain finds all chained pipe() and .pipe() calls
// (outer pipe whose subject is also a pipe call), returning matches with
// the diagnostic and both parsed results.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_unnecessary_pipe_chain(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<UnnecessaryPipeChainMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<UnnecessaryPipeChainMatch>,
    ) -> bool {
        if n.kind() == Kind::CallExpression
            && let Some(result) = tp.parse_pipe_call(n)
            && let Some(inner) = tp.parse_pipe_call(result.subject())
            && pipe_merge_supports_arity(tp, &inner, &result)
        {
            matches.push(UnnecessaryPipeChainMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, result.node()),
                outer: result,
                inner,
            });
        }

        n.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/unnecessary_pipe_chain.go:79 pipeMergeSupportsArity
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`). Go passes a nil callee type on to
// `GetSignaturesOfType`; here a callee without a type has no signatures.
pub(crate) fn pipe_merge_supports_arity(
    tp: &mut TypeParser<'_>,
    inner: &ParsedPipeCallResult,
    outer: &ParsedPipeCallResult,
) -> bool {
    if contains_spread_argument(inner.args()) || contains_spread_argument(outer.args()) {
        return false;
    }

    let mut argument_count = inner.args().len() + outer.args().len();
    if inner.kind() == TransformationKind::Pipe {
        argument_count += 1; // Standalone pipe retains its subject as the first argument.
    }

    let callee_type = tp.get_type_at_location(inner.node().as_call_expression().expression());
    let Some(callee_type) = callee_type else {
        return false;
    };
    for &signature in tp.checker().get_signatures_of_type(callee_type, SignatureKind::Call) {
        if argument_count < signature.min_argument_count() as i64 {
            continue;
        }
        if signature.has_rest_parameter() || argument_count <= signature.parameters().len() as i64 {
            return true;
        }
    }
    false
}

// go: internal/rules/unnecessary_pipe_chain.go:101 containsSpreadArgument
pub(crate) fn contains_spread_argument(args: Slice<'_, P<Node>>) -> bool {
    for (_, arg) in args.all() {
        if arg.kind() == Kind::SpreadElement {
            return true;
        }
    }
    false
}
