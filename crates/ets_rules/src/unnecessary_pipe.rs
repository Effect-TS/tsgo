// Package rules contains all Effect diagnostic rule implementations.

use std::sync::{Arc, LazyLock};

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{ParsedPipeCallResult, TypeParser};

// go: internal/rules/unnecessary_pipe.go:17 UnnecessaryPipe fcf77abc6c2df156
// UnnecessaryPipe detects pipe() and .pipe() calls with no transformation
// arguments and suggests removing the unnecessary pipe wrapper.
pub static UNNECESSARY_PIPE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "unnecessaryPipe",
    group: "style",
    description: "Removes pipe calls with no arguments",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::THIS_PIPE_CALL_CONTAINS_NO_ARGUMENTS_EFFECT_UNNECESSARY_PIPE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_unnecessary_pipe(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                &tsdiag::THIS_PIPE_CALL_CONTAINS_NO_ARGUMENTS_EFFECT_UNNECESSARY_PIPE,
                &[],
                &[],
            ));
        }
        diags
    },
});

// go: internal/rules/unnecessary_pipe.go:36 UnnecessaryPipeMatch 04930eb6d9ea786c
// UnnecessaryPipeMatch holds the diagnostic and parsed pipe call result needed
// by both the diagnostic rule and the quick-fix.
#[derive(Clone)]
pub struct UnnecessaryPipeMatch {
    pub source_file: P<SourceFile>, // The source file where this match was found
    pub location: TextRange,        // The pre-computed error range for this match
    pub result: Arc<ParsedPipeCallResult>, // The parsed pipe call (contains Subject node and call Node)
}

// go: internal/rules/unnecessary_pipe.go:44 AnalyzeUnnecessaryPipe 6788ac26f39f19d1
// AnalyzeUnnecessaryPipe finds all pipe() and .pipe() calls with no transformation
// arguments, returning matches with both the diagnostic and the parsed result.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); Go ignores it, so it is dropped.
pub fn analyze_unnecessary_pipe(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<UnnecessaryPipeMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<UnnecessaryPipeMatch>,
    ) -> bool {
        if n.kind() == Kind::CallExpression {
            if let Some(result) = tp.parse_pipe_call(n) {
                if result.args().len() == 0 {
                    matches.push(UnnecessaryPipeMatch {
                        source_file: sf,
                        location: ets_shim::scanner::get_error_range_for_node(sf, result.node()),
                        result,
                    });
                }
            }
        }

        n.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}
