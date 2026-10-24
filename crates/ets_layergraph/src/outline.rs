use ets_graph::{Graph, NodeIndex, Outgoing};
use ets_shim::ast::Symbol;
use ets_shim::checker::Type;
use ets_shim::core::P;
use ets_typeparser::TypeParser;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::*;

// go: internal/layergraph/outline.go:15 ExtractOutlineGraph
// ExtractOutlineGraph builds a simplified dependency graph from the full layer graph.
// It keeps only leaf nodes (deduplicated by symbol) and connects them based on
// type compatibility: if node A requires a service that node B provides, add an edge.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`). Go returns a `*Graph` that is never nil;
// the graph is returned by value.
pub fn extract_outline_graph(
    tp: &mut TypeParser<'_>,
    layer_graph: &Graph<LayerGraphNodeInfo, LayerGraphEdgeInfo>,
) -> Graph<LayerOutlineGraphNodeInfo, ()> {
    let mut g = ets_graph::new::<LayerOutlineGraphNodeInfo, ()>();

    // Track providers: providedType → list of outline node indices that provide it.
    let mut providers: FxHashMap<P<Type>, Vec<NodeIndex>> = FxHashMap::default();
    // Track seen symbols for deduplication.
    let mut known_symbols: FxHashSet<P<Symbol>> = FxHashSet::default();

    // Step 1: Get leaf nodes (nodes with no outgoing edges) and deduplicate by symbol.
    let mut deduped_leaf_nodes: Vec<&LayerGraphNodeInfo> = Vec::new();
    for (_, leaf_node) in layer_graph.externals(Outgoing) {
        match tp.get_symbol_at_location(leaf_node.node) {
            None => deduped_leaf_nodes.push(leaf_node),
            Some(sym) => {
                if known_symbols.insert(sym) {
                    deduped_leaf_nodes.push(leaf_node);
                }
            }
        }
    }

    // Step 2: Create outline nodes and build the provider map.
    for leaf_node in deduped_leaf_nodes {
        let node_index = g.add_node(LayerOutlineGraphNodeInfo {
            node: leaf_node.node,
            display_node: leaf_node.display_node,
            provides: leaf_node.provides.clone(),
            actual_provides: leaf_node.actual_provides.clone(),
            requires: leaf_node.requires.clone(),
        });
        for &provided_type in &leaf_node.actual_provides {
            providers.entry(provided_type).or_default().push(node_index);
        }
    }

    // Sort provider types alphabetically for deterministic edge ordering. This
    // intentionally diverges from the .repos reference implementation which uses
    // non-deterministic checker iteration order, to avoid flaky output from Go's
    // map iteration.
    let mut sorted_provider_types: Vec<P<Type>> = Vec::new();
    for &t in providers.keys() {
        sorted_provider_types.push(t);
    }
    ets_go::sort::slice(&mut sorted_provider_types, |&a, &b| {
        tp.checker().type_to_string_exported(a) < tp.checker().type_to_string_exported(b)
    });

    // Step 3: Connect requires to providers based on type assignability.
    //
    // Rust: Go ranges over `g.Nodes()` while adding edges (never nodes); the node list is taken first.
    let nodes: Vec<(NodeIndex, Vec<P<Type>>)> =
        g.nodes().map(|(index, info)| (index, info.requires.clone())).collect();
    for (node_index, requires) in nodes {
        for &required_type in &requires {
            for &provided_type in &sorted_provider_types {
                let provider_node_indices = &providers[&provided_type];
                if required_type == provided_type
                    || tp.checker().is_type_assignable_to(required_type, provided_type)
                {
                    for &provider_node_index in provider_node_indices {
                        if !g.has_edge(node_index, provider_node_index) {
                            g.add_edge(node_index, provider_node_index, ());
                        }
                    }
                }
            }
        }
    }

    g
}
