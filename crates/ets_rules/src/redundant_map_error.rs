use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile, Symbol};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectContextFlags, PipingFlow, TypeParser};
use rustc_hash::FxHashMap;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::This_generator_applies_the_same_inline_Effect_mapError_to_every_yielded_effect_Keep_that_Effect_mapError_inline_and_hoist_it_once_to_the_generator_result_Colon_Effect_gen_pipe_Effect_mapError_or_Effect_fn_function_Asterisk_Effect_mapError_effect_redundantMapError;

/// Rust-only: the rule's message (Go spells it out at each use).
static RELATED_MESSAGE: &Message =
    &tsdiag::This_repeated_Effect_mapError_site_participates_in_the_hoistable_generator_wide_mapping_effect_redundantMapError;

// go: internal/rules/redundant_map_error.go:14 RedundantMapError
pub static REDUNDANT_MAP_ERROR: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "redundantMapError",
    group: "style",
    description: "Suggests hoisting a repeated trailing Effect.mapError from every yield in an Effect generator",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code(), RELATED_MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_redundant_generator_map_error(ctx.type_parser, Some(source_file));
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            let related = redundant_map_error_related_information(ctx, m);
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &related, &[]));
        }
        diags
    },
});

// go: internal/rules/redundant_map_error.go:39 RedundantGeneratorMapErrorMatch
//
// Rust: Go's `*ast.FunctionExpression` is its node.
#[derive(Clone)]
pub struct RedundantGeneratorMapErrorMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub generator_call_node: P<Node>,
    pub generator_function: P<Node>,
    pub map_error_nodes: Vec<P<Node>>,
    pub mapper_node: P<Node>,
    pub yield_expressions: Vec<P<Node>>,
}

// go: internal/rules/redundant_map_error.go:49 redundantMapErrorCandidate
#[derive(Clone)]
pub(crate) struct RedundantMapErrorCandidate {
    pub(crate) yield_expression: P<Node>,
    pub(crate) map_error_node: P<Node>,
    pub(crate) mapper_node: P<Node>,
    pub(crate) mapper_text: String,
}

// go: internal/rules/redundant_map_error.go:56 AnalyzeRedundantGeneratorMapError
//
// Rust: the checker is the parser's (Go's nil checks of the parser and checker have no counterpart). Go ranges
// over its `candidatesByGenerator` map, so the order of the matches is random there; here the generators come in
// the order their first candidate was found. Every consumer orders by position (the diagnostics are sorted).
pub fn analyze_redundant_generator_map_error(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<RedundantGeneratorMapErrorMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut generator_calls: FxHashMap<P<Node>, P<Node>> = FxHashMap::default();
    let mut candidates_by_generator: Vec<(
        P<Node>,
        FxHashMap<P<Node>, RedundantMapErrorCandidate>,
    )> = Vec::new();

    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        let Some((candidate, gen_fn, gen_call_node)) =
            analyze_redundant_map_error_flow_candidate(tp, sf, Some(flow))
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
        if let Some(m) = analyze_redundant_generator_map_error_candidate(
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

// go: internal/rules/redundant_map_error.go:89 analyzeRedundantMapErrorFlowCandidate
//
// Rust: Go's `(candidate, genFn, genCallNode, ok)` is an `Option`.
pub(crate) fn analyze_redundant_map_error_flow_candidate(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    flow: Option<&PipingFlow>,
) -> Option<(RedundantMapErrorCandidate, P<Node>, P<Node>)> {
    let flow = flow?;
    if flow.transformations().len() == 0 {
        return None;
    }

    let last_transform = flow.transformations().at(flow.transformations().len() - 1);
    if last_transform.args().len() == 0 {
        return None;
    }

    let yield_expr = enclosing_yield_expression(Some(flow.node()))?;
    let yield_expression = yield_expr.as_yield_expression();
    if yield_expression.expression() != Some(flow.node())
        || yield_expression.asterisk_token().is_none()
    {
        return None;
    }

    let mapper_node = unwrap_transparent_expression(Some(*last_transform.args().at(0)));
    let mapper_text = node_source_text(Some(sf), mapper_node);
    let mapper_node = mapper_node?;
    if mapper_text.is_empty() {
        return None;
    }
    if !tp.is_node_reference_to_effect_module_api(last_transform.callee(), "mapError") {
        return None;
    }
    if !tp.get_effect_context_flags(yield_expr).intersects(EffectContextFlags::CanYieldEffect) {
        return None;
    }

    let gen_fn = tp.get_effect_yield_generator_function(yield_expr)?;

    let gen_call_node = enclosing_generator_call(Some(gen_fn))?;

    Some((
        RedundantMapErrorCandidate {
            yield_expression: yield_expr,
            map_error_node: last_transform.callee(),
            mapper_node,
            mapper_text,
        },
        gen_fn,
        gen_call_node,
    ))
}

// go: internal/rules/redundant_map_error.go:134 analyzeRedundantGeneratorMapErrorCandidate
//
// Rust: `(match, ok)` is an `Option`; the checker is the parser's.
pub(crate) fn analyze_redundant_generator_map_error_candidate(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    generator_call_node: Option<P<Node>>,
    generator_function: Option<P<Node>>,
    candidates: &FxHashMap<P<Node>, RedundantMapErrorCandidate>,
) -> Option<RedundantGeneratorMapErrorMatch> {
    let (Some(generator_call_node), Some(generator_function)) =
        (generator_call_node, generator_function)
    else {
        return None;
    };

    let generator_body = generator_function.as_function_expression().body()?;

    let mut yield_expressions = Vec::new();
    let mut representative_map_error: Option<P<Node>> = None;
    let mut map_error_nodes = Vec::new();
    let mut representative_mapper: Option<P<Node>> = None;
    let mut representative_text = String::new();
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
        if !mapper_can_be_hoisted(
            tp,
            Some(generator_function),
            Some(generator_call_node),
            Some(candidate.mapper_node),
        ) {
            valid_yield_count = -1;
            return true;
        }

        if representative_map_error.is_none() {
            representative_map_error = Some(candidate.map_error_node);
            representative_mapper = Some(candidate.mapper_node);
            representative_text.clone_from(&candidate.mapper_text);
        } else if candidate.mapper_text != representative_text {
            valid_yield_count = -1;
            return true;
        }

        yield_expressions.push(expr);
        map_error_nodes.push(candidate.map_error_node);
        valid_yield_count += 1;
        false
    });

    if valid_yield_count < 2
        || valid_yield_count != candidates.len() as i64
        || representative_map_error.is_none()
    {
        return None;
    }
    let mapper_node = representative_mapper?;

    Some(RedundantGeneratorMapErrorMatch {
        source_file: sf,
        location: generator_function_keyword_range(sf, generator_function),
        generator_call_node,
        generator_function,
        map_error_nodes,
        mapper_node,
        yield_expressions,
    })
}

// go: internal/rules/redundant_map_error.go:203 redundantMapErrorRelatedInformation
pub(crate) fn redundant_map_error_related_information(
    ctx: &Context<'_, '_>,
    m: &RedundantGeneratorMapErrorMatch,
) -> Vec<P<Diagnostic>> {
    if m.map_error_nodes.is_empty() {
        return Vec::new();
    }

    let mut related = Vec::with_capacity(m.map_error_nodes.len());
    for &node in &m.map_error_nodes {
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

// go: internal/rules/redundant_map_error.go:223 enclosingYieldExpression
//
// Rust: Go's `*ast.YieldExpression` is its node.
pub(crate) fn enclosing_yield_expression(node: Option<P<Node>>) -> Option<P<Node>> {
    ast::find_ancestor(node, |current| current.kind() == Kind::YieldExpression)
}

// go: internal/rules/redundant_map_error.go:233 enclosingGeneratorCall
pub(crate) fn enclosing_generator_call(node: Option<P<Node>>) -> Option<P<Node>> {
    let mut current = node;
    while let Some(c) = current {
        if c.kind() != Kind::CallExpression {
            current = c.parent();
            continue;
        }
        return Some(c);
    }
    None
}

// go: internal/rules/redundant_map_error.go:243 mapperCanBeHoisted
//
// Rust: the checker is the parser's.
pub(crate) fn mapper_can_be_hoisted(
    tp: &mut TypeParser<'_>,
    generator_function_node: Option<P<Node>>,
    hoist_location: Option<P<Node>>,
    mapper_node: Option<P<Node>>,
) -> bool {
    let (Some(generator_function_node), Some(hoist_location), Some(mapper_node)) =
        (generator_function_node, hoist_location, mapper_node)
    else {
        return false;
    };

    let mut can_hoist = true;

    // Rust: Go's recursive `walk` closure is a nested function taking what it captures.
    fn walk(
        tp: &mut TypeParser<'_>,
        generator_function_node: P<Node>,
        hoist_location: P<Node>,
        mapper_node: P<Node>,
        can_hoist: &mut bool,
        node: P<Node>,
    ) -> bool {
        if !*can_hoist {
            return *can_hoist;
        }
        if node.kind() == Kind::Identifier
            && is_hoist_sensitive_value_reference(Some(node))
            && let Some(symbol) = hoist_sensitive_reference_symbol(tp, Some(node))
        {
            if all_symbol_declarations_inside_node(Some(symbol), Some(mapper_node)) {
                node.for_each_child(&mut |child| {
                    walk(tp, generator_function_node, hoist_location, mapper_node, can_hoist, child)
                });
                return !*can_hoist;
            }

            if !all_symbol_declarations_outside_node(Some(symbol), Some(generator_function_node)) {
                *can_hoist = false;
                return true;
            }

            if !tp.checker().is_past_last_assignment(symbol, Some(hoist_location)) {
                *can_hoist = false;
                return true;
            }
        }
        node.for_each_child(&mut |child| {
            walk(tp, generator_function_node, hoist_location, mapper_node, can_hoist, child)
        });
        !*can_hoist
    }

    walk(tp, generator_function_node, hoist_location, mapper_node, &mut can_hoist, mapper_node);
    can_hoist
}

// go: internal/rules/redundant_map_error.go:281 allSymbolDeclarationsInsideNode
pub(crate) fn all_symbol_declarations_inside_node(
    symbol: Option<P<Symbol>>,
    node: Option<P<Node>>,
) -> bool {
    let (Some(symbol), Some(node)) = (symbol, node) else {
        return false;
    };
    if symbol.declarations().is_empty() {
        return false;
    }
    for &declaration in symbol.declarations() {
        if !ast::is_node_descendant_of(declaration, node) {
            return false;
        }
    }
    true
}

// go: internal/rules/redundant_map_error.go:293 allSymbolDeclarationsOutsideNode
pub(crate) fn all_symbol_declarations_outside_node(
    symbol: Option<P<Symbol>>,
    node: Option<P<Node>>,
) -> bool {
    let (Some(symbol), Some(node)) = (symbol, node) else {
        return false;
    };
    for &declaration in symbol.declarations() {
        if ast::is_node_descendant_of(declaration, node) {
            return false;
        }
    }
    true
}

// go: internal/rules/redundant_map_error.go:305 hoistSensitiveReferenceSymbol
//
// Rust: the checker is the parser's.
pub(crate) fn hoist_sensitive_reference_symbol(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> Option<P<Symbol>> {
    let node = node?;
    if let Some(parent) = node.parent()
        && parent.kind() == Kind::ShorthandPropertyAssignment
        && let Some(symbol) = tp.checker().get_shorthand_assignment_value_symbol(Some(parent))
    {
        return Some(symbol);
    }
    tp.get_symbol_at_location(node)
}

// go: internal/rules/redundant_map_error.go:317 generatorFunctionKeywordRange
//
// Rust: Go's `*ast.FunctionExpression` is its node (never nil here, as at Go's call sites).
pub(crate) fn generator_function_keyword_range(
    sf: P<SourceFile>,
    generator_function: P<Node>,
) -> TextRange {
    let Some(asterisk_token) = generator_function.as_function_expression().asterisk_token() else {
        return ets_shim::scanner::get_error_range_for_node(sf, generator_function);
    };
    let start = ets_shim::scanner::get_token_pos_of_node(generator_function, sf, false);
    let end = asterisk_token.end();
    if end < ets_shim::core::TextPos::source(start) {
        return ets_shim::scanner::get_error_range_for_node(sf, generator_function);
    }
    TextRange::from_text_pos(ets_shim::core::TextPos::source(start), end)
}

// go: internal/rules/redundant_map_error.go:329 isHoistSensitiveValueReference
pub(crate) fn is_hoist_sensitive_value_reference(node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if node.kind() != Kind::Identifier
        || ast::is_declaration_name_or_import_property_name(node)
        || ast::is_valid_type_only_alias_use_site(node)
    {
        return false;
    }

    let Some(parent) = node.parent() else {
        return true;
    };

    match parent.kind() {
        Kind::PropertyAccessExpression => {
            let prop = parent.as_property_access_expression();
            return prop.name() != node;
        }
        Kind::QualifiedName => {
            let qn = parent.as_qualified_name();
            return qn.right() != node;
        }
        Kind::PropertyAssignment => {
            let assignment = parent.as_property_assignment();
            if assignment.name() == node && assignment.initializer() != node {
                return false;
            }
        }
        _ => {}
    }

    true
}

// go: internal/rules/redundant_map_error.go:356 unwrapTransparentExpression
pub(crate) fn unwrap_transparent_expression(node: Option<P<Node>>) -> Option<P<Node>> {
    let mut node = node;
    while let Some(n) = node {
        match n.kind() {
            Kind::ParenthesizedExpression
            | Kind::SatisfiesExpression
            | Kind::AsExpression
            | Kind::NonNullExpression
            | Kind::TypeAssertionExpression => node = n.expression(),
            _ => return Some(n),
        }
    }
    None
}

// go: internal/rules/redundant_map_error.go:368 nodeSourceText
pub(crate) fn node_source_text(sf: Option<P<SourceFile>>, node: Option<P<Node>>) -> String {
    let (Some(sf), Some(node)) = (sf, node) else {
        return String::new();
    };
    let text = sf.text();
    let pos = i64::from(ets_shim::scanner::get_token_pos_of_node(node, sf, false));
    let end = node.end().source_offset().map_or(-1, i64::from);
    if pos < 0 || end < pos || end > text.len() as i64 {
        return String::new();
    }
    text[pos as usize..end as usize].to_string()
}
