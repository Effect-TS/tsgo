use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectContextFlags, PipingFlow, TypeParser};
use rustc_hash::FxHashMap;

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::This_generator_applies_Effect_orDie_to_every_yielded_effect_Hoist_it_once_to_the_generator_result_Colon_Effect_gen_pipe_Effect_orDie_or_Effect_fn_function_Asterisk_Effect_orDie_effect_redundantOrDie;

/// Rust-only: the rule's message (Go spells it out at each use).
static RELATED_MESSAGE: &Message =
    &tsdiag::This_repeated_Effect_orDie_site_participates_in_the_hoistable_generator_wide_orDie_effect_redundantOrDie;

// go: internal/rules/redundant_or_die.go:14 RedundantOrDie
pub static REDUNDANT_OR_DIE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "redundantOrDie",
    group: "style",
    description: "Suggests hoisting a repeated trailing Effect.orDie from every yield in an Effect generator",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code(), RELATED_MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_redundant_generator_or_die(ctx.type_parser, Some(source_file));
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            let related = redundant_or_die_related_information(ctx, m);
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &related, &[]));
        }
        diags
    },
});

// go: internal/rules/redundant_or_die.go:39 RedundantGeneratorOrDieMatch
//
// Rust: Go's `*ast.FunctionExpression` is its node.
#[derive(Clone)]
pub struct RedundantGeneratorOrDieMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub generator_call_node: P<Node>,
    pub generator_function: P<Node>,
    pub or_die_nodes: Vec<P<Node>>,
    pub yield_expressions: Vec<P<Node>>,
}

// go: internal/rules/redundant_or_die.go:48 redundantOrDieCandidate
#[derive(Clone, Copy)]
pub(crate) struct RedundantOrDieCandidate {
    pub(crate) yield_expression: P<Node>,
    pub(crate) or_die_node: P<Node>,
}

// go: internal/rules/redundant_or_die.go:53 AnalyzeRedundantGeneratorOrDie
//
// Rust: the checker is the parser's (Go's nil checks of the parser and checker have no counterpart). Go ranges
// over its `candidatesByGenerator` map, so the order of the matches is random there; here the generators come in
// the order their first candidate was found. Every consumer orders by position (the diagnostics are sorted).
pub fn analyze_redundant_generator_or_die(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<RedundantGeneratorOrDieMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut generator_calls: FxHashMap<P<Node>, P<Node>> = FxHashMap::default();
    let mut candidates_by_generator: Vec<(P<Node>, FxHashMap<P<Node>, RedundantOrDieCandidate>)> =
        Vec::new();

    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        let Some((candidate, gen_fn, gen_call_node)) =
            analyze_redundant_or_die_flow_candidate(tp, sf, Some(flow))
        else {
            continue;
        };
        generator_calls.insert(gen_fn, gen_call_node);
        let index = if let Some(index) =
            candidates_by_generator.iter().position(|(key, _)| *key == gen_fn)
        {
            index
        } else {
            candidates_by_generator.push((gen_fn, FxHashMap::default()));
            candidates_by_generator.len() - 1
        };
        candidates_by_generator[index].1.insert(candidate.yield_expression, candidate);
    }

    let mut matches = Vec::new();
    for (gen_fn, candidates) in &candidates_by_generator {
        if candidates.len() < 2 {
            continue;
        }
        if let Some(m) = analyze_redundant_generator_or_die_candidate(
            tp,
            sf,
            generator_calls.get(gen_fn).copied(),
            Some(*gen_fn),
            candidates,
        ) {
            matches.push(m);
        }
    }

    matches
}

// go: internal/rules/redundant_or_die.go:86 analyzeRedundantOrDieFlowCandidate
//
// Rust: Go's `(candidate, genFn, genCallNode, ok)` is an `Option`.
pub(crate) fn analyze_redundant_or_die_flow_candidate(
    tp: &mut TypeParser<'_>,
    _sf: P<SourceFile>,
    flow: Option<&PipingFlow>,
) -> Option<(RedundantOrDieCandidate, P<Node>, P<Node>)> {
    let flow = flow?;
    if flow.transformations().len() == 0 {
        return None;
    }

    let last_transform = flow.transformations().at(flow.transformations().len() - 1);
    let yield_expr = enclosing_yield_expression(Some(flow.node()))?;
    let yield_expression = yield_expr.as_yield_expression();
    if yield_expression.expression() != Some(flow.node())
        || yield_expression.asterisk_token().is_none()
    {
        return None;
    }
    if !tp.is_node_reference_to_effect_module_api(last_transform.callee(), "orDie") {
        return None;
    }
    if !tp.get_effect_context_flags(yield_expr).intersects(EffectContextFlags::CanYieldEffect) {
        return None;
    }

    let gen_fn = tp.get_effect_yield_generator_function(yield_expr)?;

    let gen_call_node = enclosing_generator_call(Some(gen_fn))?;

    Some((
        RedundantOrDieCandidate {
            yield_expression: yield_expr,
            or_die_node: last_transform.callee(),
        },
        gen_fn,
        gen_call_node,
    ))
}

// go: internal/rules/redundant_or_die.go:122 analyzeRedundantGeneratorOrDieCandidate
//
// Rust: `(match, ok)` is an `Option`.
pub(crate) fn analyze_redundant_generator_or_die_candidate(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    generator_call_node: Option<P<Node>>,
    generator_function: Option<P<Node>>,
    candidates: &FxHashMap<P<Node>, RedundantOrDieCandidate>,
) -> Option<RedundantGeneratorOrDieMatch> {
    let (Some(generator_call_node), Some(generator_function)) =
        (generator_call_node, generator_function)
    else {
        return None;
    };

    let generator_body = generator_function.as_function_expression().body()?;

    let mut yield_expressions = Vec::new();
    let mut or_die_nodes = Vec::new();
    let mut valid_yield_count: i64 = 0;

    ets_shim::checker::for_each_yield_expression(generator_body, |expr| {
        if expr.kind() != Kind::YieldExpression {
            return false;
        }
        let yield_expr = expr.as_yield_expression();
        if yield_expr.asterisk_token().is_none() || yield_expr.expression().is_none() {
            return false;
        }
        if !tp.get_effect_context_flags(expr).intersects(EffectContextFlags::CanYieldEffect)
            || tp.get_effect_yield_generator_function(expr) != Some(generator_function)
        {
            return false;
        }

        let Some(candidate) = candidates.get(&expr) else {
            valid_yield_count = -1;
            return true;
        };

        yield_expressions.push(expr);
        or_die_nodes.push(candidate.or_die_node);
        valid_yield_count += 1;
        false
    });

    if valid_yield_count < 2
        || valid_yield_count != candidates.len() as i64
        || or_die_nodes.is_empty()
    {
        return None;
    }

    Some(RedundantGeneratorOrDieMatch {
        source_file: sf,
        location: generator_function_keyword_range(sf, generator_function),
        generator_call_node,
        generator_function,
        or_die_nodes,
        yield_expressions,
    })
}

// go: internal/rules/redundant_or_die.go:174 redundantOrDieRelatedInformation
pub(crate) fn redundant_or_die_related_information(
    ctx: &Context<'_, '_>,
    m: &RedundantGeneratorOrDieMatch,
) -> Vec<P<Diagnostic>> {
    if m.or_die_nodes.is_empty() {
        return Vec::new();
    }

    let mut related = Vec::with_capacity(m.or_die_nodes.len());
    for &node in &m.or_die_nodes {
        related.push(ctx.new_diagnostic(
            m.source_file,
            ets_shim::scanner::get_error_range_for_node(m.source_file, node),
            RELATED_MESSAGE,
            &[],
            &[],
        ));
    }
    related
}
