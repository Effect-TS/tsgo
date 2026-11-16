use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{PipingFlow, PipingFlowTransformation, TransformationKind, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_FOR_EACH_EXPRESSES_THIS_EFFECTFUL_ARRAY_MAPPING_MORE_DIRECTLY_THAN_EFFECT_ALL_OVER_ARRAY_MAP_EFFECT_ALL_OF_MAP_TO_FOR_EACH;

// go: internal/rules/all_of_map_to_for_each.go:18 AllOfMapToForEach fb48d4a0e7e32674
// AllOfMapToForEach suggests using Effect.forEach instead of constructing an
// intermediate array of effects with Array#map and passing it to Effect.all.
pub static ALL_OF_MAP_TO_FOR_EACH: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "allOfMapToForEach",
    group: "style",
    description: "Suggests using Effect.forEach instead of Effect.all over an effectful Array#map",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_all_of_map_to_for_each(ctx.type_parser, source_file);
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/all_of_map_to_for_each.go:43 AllOfMapToForEachMatch b3ddbe0f6440e39a
// AllOfMapToForEachMatch holds the nodes needed by the diagnostic and its fix.
#[derive(Clone)]
pub struct AllOfMapToForEachMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    // CallNode is the standalone Effect.all call the quick fix rewrites. It is
    // nil for data-last Effect.all references inside piping flows, which are
    // diagnostic-only, since the fix would have to restructure the surrounding
    // pipe.
    pub call_node: Option<P<Node>>,
    pub receiver: P<Node>,
    // Callback is the argument of the Array#map call.
    pub callback: P<Node>,
    pub options: Option<P<Node>>,
}

// go: internal/rules/all_of_map_to_for_each.go:60 AnalyzeAllOfMapToForEach a0cded4af8ecd8d8
// AnalyzeAllOfMapToForEach finds Effect.all over an effectful Array#map whose
// receiver is array-like, in both the direct data-first form and the data-last
// form expressed through piping flows (e.g. pipe(xs.map(f), Effect.all)).
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_all_of_map_to_for_each(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<AllOfMapToForEachMatch> {
    let mut matches = Vec::new();

    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        // Direct calls: Effect.all(xs.map(f), options?)
        for (_, transformation) in flow.transformations().all() {
            if transformation.kind() != TransformationKind::Call
                && transformation.kind() != TransformationKind::CallWithOptionals
            {
                continue;
            }
            if let Some(call_node) = transformation_application_node(Some(transformation))
                && let Some(m) = analyze_all_of_map_to_for_each_call(tp, sf, call_node)
            {
                matches.push(m);
            }
        }

        // Data-last Effect.all references, e.g. pipe(xs.map(f), Effect.all).
        let sequences = PipingFlow::find_transformation_sequences(
            Some(flow),
            tp,
            &[
                &|_: &mut TypeParser<'_>, _: &PipingFlowTransformation| true,
                &|tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
                    transformation.args().len() == 0
                        && (transformation.kind() == TransformationKind::Pipe
                            || transformation.kind() == TransformationKind::Pipeable)
                        && tp.is_node_reference_to_effect_module_api(transformation.callee(), "all")
                },
            ],
        );
        for sequence in sequences.as_deref().unwrap_or_default() {
            let transformation = flow.transformations().at(sequence.start() + 1);
            let map_node =
                transformation_application_node(Some(flow.transformations().at(sequence.start())));
            if let Some(m) = analyze_all_of_map_to_for_each_receiver(
                tp,
                sf,
                transformation.callee(),
                map_node,
                None,
            ) {
                matches.push(m);
            }
        }
    }

    // Rust: Go's `sort.Slice` is unstable; the positions are unique (each match is at a distinct Effect.all
    // reference), so a stable sort gives the same order.
    matches.sort_by_key(|m| m.location.pos());
    matches
}

// go: internal/rules/all_of_map_to_for_each.go:103 transformationApplicationNode 1ea2e5b5ff68153d
// transformationApplicationNode returns the call expression that applies the
// transformation's callee, when that application is represented in the tree.
// Bare pipeable references have no application of their own and return nil.
pub(crate) fn transformation_application_node(
    transformation: Option<&PipingFlowTransformation>,
) -> Option<P<Node>> {
    let transformation = transformation?;
    let parent = transformation.callee().parent()?;
    if !ast::is_call_expression(parent) {
        return None;
    }
    let call = parent.as_call_expression();
    if call.expression() != transformation.callee() {
        return None;
    }
    Some(parent)
}

// go: internal/rules/all_of_map_to_for_each.go:115 analyzeAllOfMapToForEachCall 75400ef36a5b32c1
pub(crate) fn analyze_all_of_map_to_for_each_call(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<AllOfMapToForEachMatch> {
    if node.kind() != Kind::CallExpression {
        return None;
    }
    let all_call = node.as_call_expression();
    let args = all_call.arguments().nodes();
    if args.is_empty() || args.len() > 2 {
        return None;
    }
    if contains_spread_element(args)
        || !tp.is_node_reference_to_effect_module_api(all_call.expression(), "all")
    {
        return None;
    }

    let mut options = None;
    if args.len() == 2 {
        options = Some(args[1]);
    }
    let mut m = analyze_all_of_map_to_for_each_receiver(
        tp,
        sf,
        all_call.expression(),
        Some(args[0]),
        options,
    )?;
    m.call_node = Some(node);
    Some(m)
}

// go: internal/rules/all_of_map_to_for_each.go:141 analyzeAllOfMapToForEachReceiver 10f0382805f3d508
// analyzeAllOfMapToForEachReceiver validates the xs.map(f) shape feeding an
// Effect.all node and assembles the match.
pub(crate) fn analyze_all_of_map_to_for_each_receiver(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    all_node: P<Node>,
    map_node: Option<P<Node>>,
    options: Option<P<Node>>,
) -> Option<AllOfMapToForEachMatch> {
    let map_node = ast::skip_parentheses(map_node?);
    if map_node.kind() != Kind::CallExpression {
        return None;
    }
    let map_call = map_node.as_call_expression();
    if map_call.expression().kind() != Kind::PropertyAccessExpression
        || map_call.arguments().nodes().len() != 1
        || contains_spread_element(map_call.arguments().nodes())
    {
        return None;
    }
    let map_access = map_call.expression().as_property_access_expression();
    if map_access.name().text() != "map" {
        return None;
    }

    let receiver_type = tp.get_type_at_location(map_access.expression());
    tp.array_type(receiver_type)?;

    let callback = map_call.arguments().nodes()[0];
    let map_result_type = tp.get_type_at_location(map_node)?;
    let number_index_type = tp.checker().get_number_index_type(map_result_type);
    tp.effect_type(number_index_type)?;

    if let Some(options) = options {
        let options_type = tp.get_type_at_location(options);
        if let Some(options_type) = options_type {
            if tp.checker().get_property_of_type_exported(options_type, "mode").is_some() {
                return None;
            }
        }
    }

    Some(AllOfMapToForEachMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, all_node),
        call_node: None,
        receiver: map_access.expression(),
        callback,
        options,
    })
}

// go: internal/rules/all_of_map_to_for_each.go:188 containsSpreadElement 5bbb24f7dcb59299
pub(crate) fn contains_spread_element(nodes: &[P<Node>]) -> bool {
    for node in nodes {
        if node.kind() == Kind::SpreadElement {
            return true;
        }
    }
    false
}
