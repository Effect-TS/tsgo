use std::collections::VecDeque;

use ets_graph::{Graph, Incoming, NodeIndex, Outgoing};
use ets_shim::checker::{Checker, Type};
use ets_shim::core::P;
use rustc_hash::FxHashSet;

use crate::*;

// go: internal/layergraph/providers.go:17 walkLeavesMatching
// walkLeavesMatching performs a BFS from the given start node indices, walking
// outgoing edges. For each node, it checks if any outgoing neighbors match the
// predicate. If matching neighbors exist, it continues walking to those neighbors.
// If no matching neighbors exist and the current node matches the predicate,
// the current node is emitted as a "leaf match". This finds the deepest nodes
// in the graph that match the predicate.
pub(crate) fn walk_leaves_matching(
    g: &Graph<LayerGraphNodeInfo, LayerGraphEdgeInfo>,
    start_indices: &[NodeIndex],
    predicate: &mut dyn FnMut(&LayerGraphNodeInfo) -> bool,
) -> Vec<LayerGraphNodeInfo> {
    let mut result = Vec::new();
    let mut queue: VecDeque<NodeIndex> = VecDeque::with_capacity(start_indices.len());
    queue.extend(start_indices.iter().copied());
    let mut discovered: FxHashSet<NodeIndex> = FxHashSet::default();

    while let Some(current) = queue.pop_front() {
        if !discovered.insert(current) {
            continue;
        }

        let neighbors = g.neighbors_directed(current, Outgoing);
        let mut matching_neighbors = Vec::new();
        for neighbor in neighbors {
            if let Some(neighbor_data) = g.get_node(neighbor)
                && predicate(neighbor_data)
            {
                matching_neighbors.push(neighbor);
            }
        }

        if !matching_neighbors.is_empty() {
            queue.extend(matching_neighbors);
        } else if let Some(node_data) = g.get_node(current)
            && predicate(node_data)
        {
            result.push(node_data.clone());
        }
    }

    result
}

// go: internal/layergraph/providers.go:67 ExtractProvidersAndRequirers
// ExtractProvidersAndRequirers extracts summary information about which layers
// provide or require which services. It uses a two-step algorithm:
//
// Step 1: Get root nodes (no incoming edges) to determine WHICH types to report.
// The root node's Layer type reflects the net result of the composition, so
// internally-satisfied services are already excluded.
//
// Step 2: For each type from step 1, use walkLeavesMatching BFS to find the
// deepest leaf nodes that match, giving source location attribution.
pub fn extract_providers_and_requirers(
    c: &mut Checker,
    layer_graph: &Graph<LayerGraphNodeInfo, LayerGraphEdgeInfo>,
) -> Vec<ProviderRequirerInfo> {
    // Step 1: Get root nodes (externals with no incoming edges).
    let mut root_nodes: Vec<&LayerGraphNodeInfo> = Vec::new();
    let mut root_node_indices: Vec<NodeIndex> = Vec::new();
    for (idx, node) in layer_graph.externals(Incoming) {
        root_nodes.push(node);
        root_node_indices.push(idx);
    }

    // Collect unique Provides and Requires types from root nodes.
    let mut provides_set: FxHashSet<P<Type>> = FxHashSet::default();
    let mut requires_set: FxHashSet<P<Type>> = FxHashSet::default();
    for root in &root_nodes {
        for &t in &root.provides {
            provides_set.insert(t);
        }
        for &t in &root.requires {
            requires_set.insert(t);
        }
    }

    let mut result = Vec::new();

    // Step 2: For each type, walk leaves to find attribution.
    let mut walk_types = |type_set: &FxHashSet<P<Type>>, kind: ProviderRequirerKind| {
        // Sort types alphabetically for deterministic output. This intentionally
        // diverges from the .repos reference implementation which uses non-deterministic
        // checker iteration order, to avoid flaky output from Go's map iteration.
        let mut types: Vec<P<Type>> = type_set.iter().copied().collect();
        ets_go::sort::slice(&mut types, |&a, &b| {
            c.type_to_string_exported(a) < c.type_to_string_exported(b)
        });

        for layer_type in types {
            let mut predicate = |node: &LayerGraphNodeInfo| {
                let type_list = if kind == ProviderRequirerKind::Provided {
                    &node.provides
                } else {
                    &node.requires
                };
                for &t in type_list {
                    if t == layer_type || c.is_type_assignable_to(t, layer_type) {
                        return true;
                    }
                }
                false
            };

            let leaf_nodes = walk_leaves_matching(layer_graph, &root_node_indices, &mut predicate);

            let mut ast_nodes = Vec::new();
            let mut display_nodes = Vec::new();
            for node in leaf_nodes {
                ast_nodes.push(node.node);
                display_nodes.push(node.display_node);
            }

            result.push(ProviderRequirerInfo {
                kind,
                type_: layer_type,
                nodes: ast_nodes,
                display_nodes,
            });
        }
    };

    walk_types(&provides_set, ProviderRequirerKind::Provided);
    walk_types(&requires_set, ProviderRequirerKind::Required);

    result
}
