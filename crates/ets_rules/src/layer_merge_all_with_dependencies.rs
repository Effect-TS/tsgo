use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;
use rustc_hash::{FxHashMap, FxHashSet};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_LAYER_PROVIDES_0_WHICH_IS_REQUIRED_BY_ANOTHER_LAYER_IN_THE_SAME_LAYER_MERGE_ALL_CALL_LAYER_MERGE_ALL_CREATES_LAYERS_IN_PARALLEL_SO_DEPENDENCIES_BETWEEN_LAYERS_WILL_NOT_BE_SATISFIED_CONSIDER_MOVING_THIS_LAYER_INTO_A_LAYER_PROVIDE_MERGE_AFTER_THE_LAYER_MERGE_ALL_EFFECT_LAYER_MERGE_ALL_WITH_DEPENDENCIES;

// go: internal/rules/layer_merge_all_with_dependencies.go:19 LayerMergeAllWithDependencies f9924feb04ba3f42
// LayerMergeAllWithDependencies detects interdependencies in Layer.mergeAll calls
// where one layer provides a service that another layer requires. Since mergeAll
// creates layers in parallel, these dependencies will not be satisfied.
pub static LAYER_MERGE_ALL_WITH_DEPENDENCIES: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "layerMergeAllWithDependencies",
    group: "antipattern",
    description: "Detects interdependencies in Layer.mergeAll calls where one layer provides a service that another layer requires",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let matches = analyze_layer_merge_all_with_dependencies(ctx.type_parser, ctx.source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&m.provided_types],
            ));
        }
        diags
    },
});

// go: internal/rules/layer_merge_all_with_dependencies.go:38 LayerMergeAllWithDependenciesMatch 41d52807ef941464
// LayerMergeAllWithDependenciesMatch holds the AST nodes needed by both the
// diagnostic rule and the quick-fix for the layerMergeAllWithDependencies pattern.
#[derive(Clone)]
pub struct LayerMergeAllWithDependenciesMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,   // The error range on the provider argument
    pub call_node: P<Node>,    // The full Layer.mergeAll(...) call expression node
    pub provider_arg: P<Node>, // The specific argument node that is the dependency provider
    pub provider_index: i64,   // Index of the provider argument in the call's argument list
    pub all_args: &'static [P<Node>], // All arguments of the mergeAll call
    pub provided_types: String, // Formatted string of provided type names
}

// go: internal/rules/layer_merge_all_with_dependencies.go:50 AnalyzeLayerMergeAllWithDependencies ebe6a1907e96c5a7
// AnalyzeLayerMergeAllWithDependencies finds all Layer.mergeAll calls with
// interdependencies where one layer provides a service required by another.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_layer_merge_all_with_dependencies(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<LayerMergeAllWithDependenciesMatch> {
    let mut matches = Vec::new();

    // Stack-based traversal
    let mut node_to_visit: Vec<P<Node>> = Vec::new();
    sf.as_node().for_each_child(&mut |child| {
        node_to_visit.push(child);
        false
    });

    while let Some(node) = node_to_visit.pop() {
        if node.kind() == Kind::CallExpression {
            let result = analyze_layer_merge_all_call(tp, sf, node);
            if !result.is_empty() {
                matches.extend(result);
            }
        }

        // Enqueue children
        node.for_each_child(&mut |child| {
            node_to_visit.push(child);
            false
        });
    }

    matches
}

// go: internal/rules/layer_merge_all_with_dependencies.go:81 layerInfo 1fedd0140154b86f
// layerInfo holds parsed layer information for a mergeAll argument.
#[derive(Clone, Copy)]
pub(crate) struct LayerInfo {
    pub(crate) arg: P<Node>,
    pub(crate) requirements_type: P<Type>,
}

// go: internal/rules/layer_merge_all_with_dependencies.go:87 analyzeLayerMergeAllCall 1ba042d3d387494b
// analyzeLayerMergeAllCall checks a call expression for Layer.mergeAll interdependencies.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`). Go ranges over `actuallyProvidedMap` (a
// map), so when one provider gives several types to the other layers, the order of the names in the message is
// Go's random map order. Here the map keeps insertion order (an assignment to an existing key keeps its place, as
// a map assignment does), which is the order Go's map yields most of the time for small maps and the only
// deterministic choice that follows the source.
pub(crate) fn analyze_layer_merge_all_call(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Vec<LayerMergeAllWithDependenciesMatch> {
    if node.kind() != Kind::CallExpression {
        return Vec::new();
    }
    let call = node.as_call_expression();

    let args = call.arguments().nodes();
    if args.len() < 2 {
        return Vec::new();
    }

    // Check if this is Layer.mergeAll
    if !tp.is_node_reference_to_effect_layer_module_api(call.expression(), "mergeAll") {
        return Vec::new();
    }

    // Parse all layer arguments
    let mut layer_infos: Vec<LayerInfo> = Vec::new();
    // Map of actually provided types -> argument node that provides them
    // Rust: an insertion-ordered map (see above).
    let mut actually_provided_map: Vec<(P<Type>, P<Node>)> = Vec::new();

    for &arg in args {
        let Some(layer) = tp.layer_type_at_location(arg) else {
            continue;
        };

        // Unroll union members for provided types (ROut)
        let provided_members = tp.unroll_union_members(layer.r_out()).unwrap_or_default();

        // Filter out never types and pass-through types
        for provided_type in provided_members {
            if provided_type.flags().intersects(TypeFlags::Never) {
                continue;
            }
            // A pass-through type is both provided (ROut) and required (RIn) by the same layer
            if tp.checker().is_type_assignable_to(provided_type, layer.r_in()) {
                continue;
            }
            if let Some(entry) = actually_provided_map.iter_mut().find(|(t, _)| *t == provided_type)
            {
                entry.1 = arg;
            } else {
                actually_provided_map.push((provided_type, arg));
            }
        }

        layer_infos.push(LayerInfo { arg, requirements_type: layer.r_in() });
    }

    // Check for interdependencies: build a map of provider -> consumed types
    #[derive(Clone, Copy)]
    struct ProviderConsumer {
        provided_type: P<Type>,
    }
    let mut provider_to_consumers: FxHashMap<P<Node>, Vec<ProviderConsumer>> = FxHashMap::default();

    for li in &layer_infos {
        for &(provided_type, provider_arg) in &actually_provided_map {
            // Skip self-references
            if provider_arg == li.arg {
                continue;
            }
            // Check if this provided type satisfies the layer's requirements
            if tp.checker().is_type_assignable_to(provided_type, li.requirements_type) {
                provider_to_consumers
                    .entry(provider_arg)
                    .or_default()
                    .push(ProviderConsumer { provided_type });
            }
        }
    }

    // Build matches for providers, iterating in argument order for determinism
    let mut matches = Vec::new();
    for (arg_index, &arg) in args.iter().enumerate() {
        let Some(consumers) = provider_to_consumers.get(&arg) else {
            continue;
        };
        if consumers.is_empty() {
            continue;
        }

        // Collect unique type names
        let mut seen: FxHashSet<String> = FxHashSet::default();
        let mut type_names: Vec<String> = Vec::new();
        for consumer in consumers {
            let name = tp.checker().type_to_string_exported(consumer.provided_type);
            if seen.insert(name.clone()) {
                type_names.push(name);
            }
        }

        matches.push(LayerMergeAllWithDependenciesMatch {
            source_file: sf,
            location: ets_shim::scanner::get_error_range_for_node(sf, arg),
            call_node: node,
            provider_arg: arg,
            provider_index: arg_index as i64,
            all_args: args,
            provided_types: type_names.join(", "),
        });
    }

    matches
}
