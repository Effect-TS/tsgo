use ets_graph::{Graph, Incoming, NodeIndex, Outgoing};
use ets_shim::checker::Type;
use ets_shim::core::P;
use ets_typeparser::TypeParser;
use rustc_hash::FxHashSet;

use crate::*;

// go: internal/layergraph/magic.go:15 ConvertOutlineGraphToLayerMagic 5640162e1809546b
// ConvertOutlineGraphToLayerMagic takes an outline graph and a set of target output types,
// then computes an ordered list of layer nodes annotated with merges/provides flags
// that determine which Layer.* combinator to use (provide, provideMerge, or merge).
//
// Rust: Go returns a `*LayerMagicResult` that is never nil; the result is returned by value. The parser is only
// read (`UnrollUnionMembers`), so it is borrowed shared.
pub fn convert_outline_graph_to_layer_magic(
    tp: &TypeParser<'_>,
    outline_graph: &Graph<LayerOutlineGraphNodeInfo, ()>,
    target_output_types: &[P<Type>],
) -> LayerMagicResult {
    // Step 1: Unroll target output types into individual types
    let mut output_types: Vec<P<Type>> = Vec::new();
    for &t in target_output_types {
        output_types.extend(tp.unroll_union_members(t).into_iter().flatten());
    }

    // Build a set of missing output types for fast lookup
    let mut missing_output_types: FxHashSet<P<Type>> = FxHashSet::default();
    for &t in &output_types {
        missing_output_types.insert(t);
    }

    // Step 2: Clone and reverse the outline graph
    let mut reversed_graph = outline_graph.clone();
    reversed_graph.reverse();

    // Step 3: Find root indices (nodes with no incoming edges in the reversed graph)
    let mut root_indices: Vec<NodeIndex> = Vec::new();
    for (idx, _) in reversed_graph.externals(Incoming) {
        root_indices.push(idx);
    }

    // Sort roots by layer shape, then provides/requires count.
    ets_go::slices::sort_func(&mut root_indices, |&a, &b| {
        compare_nodes_by_order(&reversed_graph, a, b).cmp(&0)
    });

    // Step 4: Run custom DFS post-order with ordering on the reversed graph
    let all_nodes = dfs_post_order_with_order(&reversed_graph, &root_indices);

    // Step 5: Process each visited node
    let mut result: Vec<LayerMagicNode> = Vec::new();
    for node_info in all_nodes {
        if !ets_shim::ast::is_expression(node_info.node) {
            continue;
        }

        // Determine if this node should merge (provides a type still in missingOutputTypes)
        let mut should_merge = false;
        for t in &node_info.actual_provides {
            if missing_output_types.contains(t) {
                should_merge = true;
                break;
            }
        }

        // If merging, remove actual provides from missing output types
        if should_merge {
            for t in &node_info.actual_provides {
                missing_output_types.remove(t);
            }
        }

        result.push(LayerMagicNode {
            merges: should_merge,
            provides: true,
            node: node_info.node,
            provided_types: node_info.provides,
            actual_provided_types: node_info.actual_provides,
            required_types: node_info.requires,
        });
    }

    // Step 6: Collect remaining missing output types
    let mut remaining: Vec<P<Type>> = Vec::new();
    for &t in &output_types {
        if missing_output_types.contains(&t) {
            remaining.push(t);
        }
    }

    LayerMagicResult { nodes: result, missing_output_types: remaining }
}

// go: internal/layergraph/magic.go:99 dfsPostOrderWithOrder b17b79562be3ba6b
// dfsPostOrderWithOrder performs an iterative post-order DFS with custom node ordering.
// Neighbors are sorted by layer shape, then provides/requires count.
pub(crate) fn dfs_post_order_with_order(
    g: &Graph<LayerOutlineGraphNodeInfo, ()>,
    start: &[NodeIndex],
) -> Vec<LayerOutlineGraphNodeInfo> {
    struct Entry {
        node: NodeIndex,
        expanded: bool,
    }

    let mut discovered: FxHashSet<NodeIndex> = FxHashSet::default();
    let mut result: Vec<LayerOutlineGraphNodeInfo> = Vec::new();

    // Initialize stack with start nodes in reverse order (first start node on top)
    let mut stack: Vec<Entry> = Vec::with_capacity(start.len());
    for &node in start.iter().rev() {
        stack.push(Entry { node, expanded: false });
    }

    while let Some(top) = stack.pop() {
        if discovered.contains(&top.node) {
            if top.expanded {
                // Second visit — emit post-order
                if let Some(node_data) = g.get_node(top.node) {
                    result.push(node_data.clone());
                }
            }
            continue;
        }

        // First visit — mark discovered, push back with expanded=true, then push children
        discovered.insert(top.node);
        stack.push(Entry { node: top.node, expanded: true });

        // Get neighbors in outgoing direction and sort by custom order
        let neighbors = neighbors_outgoing_sorted(g, top.node);
        for &neighbor in neighbors.iter().rev() {
            if !discovered.contains(&neighbor) {
                stack.push(Entry { node: neighbor, expanded: false });
            }
        }
    }

    result
}

// go: internal/layergraph/magic.go:149 neighborsOutgoingSorted c3186dc9caa8917f
// neighborsOutgoingSorted returns outgoing neighbor indices sorted by the layer ordering:
// layer shape, then provides/requires count.
pub(crate) fn neighbors_outgoing_sorted(
    g: &Graph<LayerOutlineGraphNodeInfo, ()>,
    node_index: NodeIndex,
) -> Vec<NodeIndex> {
    let mut neighbors = g.neighbors_directed(node_index, Outgoing);
    ets_go::slices::sort_func(&mut neighbors, |&a, &b| compare_nodes_by_order(g, a, b).cmp(&0));
    neighbors
}

// go: internal/layergraph/magic.go:162 compareNodesByOrder 2ed5e13b8e142f01
// compareNodesByOrder compares two nodes by composition priority:
// both provides/requires, only provides, neither, only requires.
//
// Rust: Go reads the zero node for a missing index; the indices always come from the graph.
pub(crate) fn compare_nodes_by_order(
    g: &Graph<LayerOutlineGraphNodeInfo, ()>,
    a: NodeIndex,
    b: NodeIndex,
) -> i64 {
    let node_a = g.get_node(a).expect("compareNodesByOrder: node in the graph");
    let node_b = g.get_node(b).expect("compareNodesByOrder: node in the graph");

    let rank_diff = layer_order_rank(node_a) - layer_order_rank(node_b);
    if rank_diff != 0 {
        return rank_diff;
    }
    // Sort by provides count descending (reverse order)
    let len_diff = node_b.provides.len() as i64 - node_a.provides.len() as i64;
    if len_diff != 0 {
        return len_diff;
    }
    // Tiebreak by requires count descending (reverse order)
    let len_diff = node_b.requires.len() as i64 - node_a.requires.len() as i64;
    if len_diff != 0 {
        return len_diff;
    }
    a - b
}

// go: internal/layergraph/magic.go:183 layerOrderRank fa601558455faba4
pub(crate) fn layer_order_rank(node: &LayerOutlineGraphNodeInfo) -> i64 {
    let has_provides = !node.provides.is_empty();
    let has_requires = !node.requires.is_empty();
    match () {
        () if has_provides && has_requires => 0,
        () if has_provides => 1,
        () if !has_requires => 2,
        () => 3,
    }
}
