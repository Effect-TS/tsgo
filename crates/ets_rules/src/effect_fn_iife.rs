// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::X_0_1_RETURNS_A_REUSABLE_FUNCTION_THAT_CAN_TAKE_ARGUMENTS_BUT_IT_IS_INVOKED_IMMEDIATELY_HERE_EFFECT_GEN_REPRESENTS_THE_IMMEDIATE_USE_FORM_FOR_THIS_PATTERN_2_EFFECT_EFFECT_FN_IIFE;

// go: internal/rules/effect_fn_iife.go:17 EffectFnIife
// EffectFnIife detects Effect.fn or Effect.fnUntraced calls that are immediately invoked (IIFE pattern).
pub static EFFECT_FN_IIFE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectFnIife",
    group: "antipattern",
    description: "Effect.fn or Effect.fnUntraced is called as an IIFE; use Effect.gen instead",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_effect_fn_iife(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            let result = &m.result;
            let mut effect_module_name = "Effect".to_string();
            if let Some(effect_module) = result.effect_module
                && effect_module.kind() == Kind::Identifier
            {
                effect_module_name = ets_shim::scanner::get_text_of_node(effect_module);
            }
            let mut with_span_hint = String::new();
            if let Some(trace_expression) = result.trace_expression {
                let pos = trace_expression.pos().expect_source("parsed node") as usize;
                let end = trace_expression.end().expect_source("parsed node") as usize;
                let trace_text = &ctx.source_file.text()[pos..end];
                with_span_hint = " with Effect.withSpan(".to_string()
                    + trace_text
                    + ") piped in the end to maintain tracing spans";
            }
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&effect_module_name, result.variant, &with_span_hint],
            ));
        }
        diags
    },
});

// go: internal/rules/effect_fn_iife.go:46 EffectFnIifeMatch
// EffectFnIifeMatch holds the parsed result needed by both the diagnostic rule
// and the quick-fix for the effectFnIife pattern.
#[derive(Clone)]
pub struct EffectFnIifeMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub result: EffectFnIifeResult,
}

// go: internal/rules/effect_fn_iife.go:52 EffectFnIifeResult
// Rust: the call and function fields are the nodes (Go's `*ast.CallExpression` / `*ast.FunctionExpression` views),
// and `pipe_arguments` is a copy of the read-only view (nil stays `None`).
#[derive(Clone)]
pub struct EffectFnIifeResult {
    pub outer_call: P<Node>,
    pub inner_call: P<Node>,
    pub effect_module: Option<P<Node>>,
    pub variant: &'static str,
    pub generator_function: Option<P<Node>>,
    pub pipe_arguments: Option<Vec<P<Node>>>,
    pub trace_expression: Option<P<Node>>,
}

// go: internal/rules/effect_fn_iife.go:64 AnalyzeEffectFnIife
// AnalyzeEffectFnIife finds all Effect.fn or Effect.fnUntraced calls that are
// immediately invoked (IIFE pattern) in the given source file.
//
// Rust: Go's checker argument is unused and dropped.
pub fn analyze_effect_fn_iife(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<EffectFnIifeMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<EffectFnIifeMatch>,
    ) -> bool {
        if let Some(result) = parse_effect_fn_iife(tp, n) {
            matches.push(EffectFnIifeMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, result.outer_call),
                result,
            });
        }

        n.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);

    matches
}

// go: internal/rules/effect_fn_iife.go:90 parseEffectFnIife
pub(crate) fn parse_effect_fn_iife(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> Option<EffectFnIifeResult> {
    if node.kind() != Kind::CallExpression {
        return None;
    }

    let outer_call = node.as_call_expression();

    let inner_node = outer_call.expression();
    if inner_node.kind() != Kind::CallExpression {
        return None;
    }

    if let Some(result) = tp.effect_fn_call(inner_node) {
        return Some(EffectFnIifeResult {
            outer_call: node,
            inner_call: inner_node,
            effect_module: result.effect_module(),
            variant: result.variant().as_str(),
            generator_function: result.generator_function(),
            pipe_arguments: result.pipe_arguments().copy(),
            trace_expression: result.trace_expression(),
        });
    }

    None
}
