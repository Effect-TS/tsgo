use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    LazyExpressionFlags, PipingFlowTransformation, TypeParser, parse_lazy_expression,
};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_EXPRESSION_DISCARDS_THE_SUCCESS_VALUE_THROUGH_MAPPING_EFFECT_AS_VOID_REPRESENTS_THAT_FORM_DIRECTLY_EFFECT_EFFECT_MAP_VOID;

// go: internal/rules/effect_map_void.go:16 EffectMapVoid
// EffectMapVoid suggests using Effect.asVoid instead of Effect.map(() => void 0),
// Effect.map(() => undefined), or Effect.map(() => {}).
pub static EFFECT_MAP_VOID: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectMapVoid",
    group: "style",
    description: "Suggests using Effect.asVoid instead of Effect.map(() => void 0), Effect.map(() => undefined), or Effect.map(() => {})",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_effect_map_void(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/effect_map_void.go:35 EffectMapVoidMatch
// EffectMapVoidMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the effectMapVoid pattern.
#[derive(Clone)]
pub struct EffectMapVoidMatch {
    pub source_file: P<SourceFile>, // The source file where the diagnostic should be reported
    pub location: TextRange,        // The pre-computed error range for this match
    pub transformation: PipingFlowTransformation,
}

// go: internal/rules/effect_map_void.go:44 isVoidCallback
// isVoidCallback checks if a callback argument is a "void callback":
// either an empty function (block body with zero statements) or a lazy void expression
// (zero-param function returning void 0, undefined, or parenthesized versions).
pub(crate) fn is_void_callback(node: P<Node>) -> bool {
    // First try parsing as a lazy expression (thunk=true, requiring zero params)
    let lazy = parse_lazy_expression(node, LazyExpressionFlags::Thunk);
    if let Some(lazy) = lazy {
        // ParseLazyExpression succeeded — check if the expression is a void expression
        return is_void_expression(Some(lazy.expression()));
    }

    // ParseLazyExpression returns nil for empty block bodies (0 statements).
    // Handle that case explicitly: arrow function or function expression with
    // zero parameters, no type parameters, and a block body with zero statements.
    let (type_parameters, parameters, body) = match node.kind() {
        Kind::ArrowFunction => {
            let f = node.as_arrow_function();
            (f.type_parameters(), f.parameters(), f.body())
        }
        Kind::FunctionExpression => {
            let f = node.as_function_expression();
            (f.type_parameters(), f.parameters(), f.body())
        }
        _ => return false,
    };
    if type_parameters.is_some_and(|l| !l.nodes().is_empty()) {
        return false;
    }
    if parameters.is_some_and(|l| !l.nodes().is_empty()) {
        return false;
    }
    if let Some(body) = body
        && body.kind() == Kind::Block
    {
        return body.as_block().statements().nodes().is_empty();
    }

    false
}

// go: internal/rules/effect_map_void.go:90 AnalyzeEffectMapVoid
// AnalyzeEffectMapVoid finds data-last and data-first Effect.map calls with void
// callbacks that can be replaced with Effect.asVoid. Driving detection off the
// piping-flow transformations (rather than a raw AST walk) normalizes the subject
// away, so both pipe(self, Effect.map(cb)) / self.pipe(Effect.map(cb)) and the
// data-first Effect.map(self, cb) are matched uniformly.
//
// Rust: Go's checker argument is unused and dropped.
pub fn analyze_effect_map_void(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<EffectMapVoidMatch> {
    let mut matches = Vec::new();

    let flows = tp.piping_flows(sf, true);
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index);
            if transformation.args().len() != 1 {
                continue;
            }
            if !is_void_callback(*transformation.args().at(0)) {
                continue;
            }
            if !tp.is_node_reference_to_effect_module_api(transformation.callee(), "map") {
                continue;
            }
            matches.push(EffectMapVoidMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
                transformation: transformation.clone(),
            });
        }
    }

    matches
}
