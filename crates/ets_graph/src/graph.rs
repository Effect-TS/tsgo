use std::collections::VecDeque;

use ets_go::fmt::{Value, sprint};
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

// go: internal/graph/graph.go:14 NodeIndex
// NodeIndex identifies a node in the graph. Indices are auto-incremented and never reused.
pub type NodeIndex = i64;

// go: internal/graph/graph.go:17 EdgeIndex
// EdgeIndex identifies an edge in the graph. Indices are auto-incremented and never reused.
pub type EdgeIndex = i64;

// go: internal/graph/graph.go:20 Direction
// go: internal/graph/graph.go:22 Outgoing
// Direction specifies the direction of traversal or query.
//
// Rust: Go's `type Direction string` with its two constants. Only the constants are ever passed, so the type is
// the enum of them (docs/PORTING.md, "Naming"); Go's empty `Direction` (a zero `TraversalConfig`) is `None` there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    // Outgoing follows edges from source to target.
    Outgoing,
    // Incoming follows edges from target to source.
    Incoming,
}

pub use Direction::{Incoming, Outgoing};

// go: internal/graph/graph.go:30 Edge
// Edge represents a directed edge between two nodes with associated data.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Edge<E> {
    pub source: NodeIndex,
    pub target: NodeIndex,
    pub data: E,
}

// go: internal/graph/graph.go:39 TraversalConfig
// TraversalConfig configures traversal algorithms (DFS, BFS, etc.).
// When Start is empty, defaults to all source nodes (nodes with no incoming edges).
// When Direction is empty, defaults to Outgoing.
#[derive(Clone, Default, Debug)]
pub struct TraversalConfig {
    pub start: Vec<NodeIndex>,
    pub direction: Option<Direction>,
}

/// Rust-only: the label function of `MermaidOptions`.
pub type MermaidLabel<'a, T> = Box<dyn Fn(&T) -> String + 'a>;

/// Rust-only: the shape function of `MermaidOptions` (open and close delimiters).
pub type MermaidShape<'a, T> = Box<dyn Fn(&T) -> (String, String) + 'a>;

// go: internal/graph/graph.go:45 MermaidOptions
// MermaidOptions configures Mermaid flowchart rendering.
//
// Rust: Go's nil label functions fall back to `fmt.Sprint`, which formats any value; Rust can only format a
// `Value`, so the labels are required fields and that fallback is `MermaidOptions::default()` (for `Value` node and
// edge data). Every Go caller outside the tests sets both labels. The shapes stay optional, as in Go.
pub struct MermaidOptions<'a, N, E> {
    pub node_label: MermaidLabel<'a, N>,
    pub node_shape: Option<MermaidShape<'a, N>>,
    pub edge_label: MermaidLabel<'a, E>,
    pub edge_shape: Option<MermaidShape<'a, E>>,
    pub direction: String,
}

impl<N: Value, E: Value> Default for MermaidOptions<'_, N, E> {
    // Rust-only: Go's nil `NodeLabel` / `EdgeLabel`, which `ToMermaid` replaces with `fmt.Sprint` (see above).
    fn default() -> Self {
        MermaidOptions {
            node_label: Box::new(|data: &N| sprint(&[data])),
            node_shape: None,
            edge_label: Box::new(|data: &E| sprint(&[data])),
            edge_shape: None,
            direction: String::new(),
        }
    }
}

// go: internal/graph/graph.go:54 isAcyclicStackEntry
// isAcyclicStackEntry is used by IsAcyclic for iterative DFS cycle detection.
#[derive(Clone, Copy)]
pub(crate) struct IsAcyclicStackEntry {
    node: NodeIndex,
    neighbor_idx: usize,
}

// go: internal/graph/graph.go:61 Graph
// Graph is a generic directed graph with typed node and edge data.
// It uses adjacency lists backed by maps for O(1) lookups.
pub struct Graph<N, E> {
    pub(crate) nodes: FxHashMap<NodeIndex, N>,
    pub(crate) edges: FxHashMap<EdgeIndex, Edge<E>>,
    pub(crate) adjacency: FxHashMap<NodeIndex, Vec<EdgeIndex>>,
    pub(crate) reverse_adjacency: FxHashMap<NodeIndex, Vec<EdgeIndex>>,
    pub(crate) next_node_index: NodeIndex,
    pub(crate) next_edge_index: EdgeIndex,
}

// go: internal/graph/graph.go:71 New
// New creates an empty directed graph.
pub fn new<N, E>() -> Graph<N, E> {
    Graph {
        nodes: FxHashMap::default(),
        edges: FxHashMap::default(),
        adjacency: FxHashMap::default(),
        reverse_adjacency: FxHashMap::default(),
        next_node_index: 0,
        next_edge_index: 0,
    }
}

// Rust-only: the nil-safe read of an adjacency list (Go's `g.adjacency[k]` is nil for a missing key).
fn adjacency_list(
    adjacency: &FxHashMap<NodeIndex, Vec<EdgeIndex>>,
    index: NodeIndex,
) -> &[EdgeIndex] {
    adjacency.get(&index).map_or(&[], Vec::as_slice)
}

// Rust-only: the map's keys sorted (Go's repeated `keys := ...; slices.Sort(keys)`).
fn sorted_keys<V>(m: &FxHashMap<i64, V>) -> Vec<i64> {
    let mut keys: Vec<i64> = Vec::with_capacity(m.len());
    for &k in m.keys() {
        keys.push(k);
    }
    keys.sort_unstable();
    keys
}

impl<N, E> Graph<N, E> {
    // Rust-only: Go's `g.nodes[node]` in the traversals. Go yields the zero value for a start index that is not a
    // node; no Go caller passes one (the traversals are only called with existing nodes), so Rust panics.
    fn node_data(&self, node: NodeIndex) -> &N {
        self.nodes.get(&node).expect("graph: traversal start node does not exist")
    }

    // go: internal/graph/graph.go:81 Graph.AddNode
    // AddNode adds a node with the given data and returns its index.
    pub fn add_node(&mut self, data: N) -> NodeIndex {
        let idx = self.next_node_index;
        self.nodes.insert(idx, data);
        self.adjacency.insert(idx, Vec::new());
        self.reverse_adjacency.insert(idx, Vec::new());
        self.next_node_index += 1;
        idx
    }

    // go: internal/graph/graph.go:92 Graph.GetNode
    // GetNode returns the data for the node at the given index.
    // The second return value is false if the node does not exist.
    pub fn get_node(&self, index: NodeIndex) -> Option<&N> {
        self.nodes.get(&index)
    }

    // go: internal/graph/graph.go:98 Graph.HasNode
    // HasNode returns true if a node with the given index exists.
    pub fn has_node(&self, index: NodeIndex) -> bool {
        self.nodes.contains_key(&index)
    }

    // go: internal/graph/graph.go:105 Graph.UpdateNode
    // UpdateNode applies fn to the data of the node at the given index.
    // If the node does not exist, this is a no-op.
    //
    // Rust: `fn` reads the data by reference (Go passes a copy).
    pub fn update_node(&mut self, index: NodeIndex, f: impl FnOnce(&N) -> N) {
        if let Some(data) = self.nodes.get_mut(&index) {
            *data = f(data);
        }
    }

    // go: internal/graph/graph.go:112 Graph.RemoveNode
    // RemoveNode removes the node at the given index and all its incident edges.
    pub fn remove_node(&mut self, index: NodeIndex) {
        if !self.has_node(index) {
            return;
        }

        // Collect all edge indices to remove (deduplicate since self-loops appear in both lists)
        let mut edge_set = FxHashSet::default();
        for &ei in adjacency_list(&self.adjacency, index) {
            edge_set.insert(ei);
        }
        for &ei in adjacency_list(&self.reverse_adjacency, index) {
            edge_set.insert(ei);
        }

        // Remove each edge from the opposite node's adjacency lists and from edges map
        for ei in edge_set {
            // Go reads the zero Edge for a missing index.
            let (source, target) =
                self.edges.get(&ei).map_or((0, 0), |edge| (edge.source, edge.target));
            if source != index {
                remove_from_slice(self.adjacency.entry(source).or_default(), ei);
            }
            if target != index {
                remove_from_slice(self.reverse_adjacency.entry(target).or_default(), ei);
            }
            self.edges.remove(&ei);
        }

        self.nodes.remove(&index);
        self.adjacency.remove(&index);
        self.reverse_adjacency.remove(&index);
    }

    // go: internal/graph/graph.go:144 Graph.NodeCount
    // NodeCount returns the number of nodes in the graph.
    pub fn node_count(&self) -> i64 {
        self.nodes.len() as i64
    }

    // go: internal/graph/graph.go:160 Graph.AddEdge
    // AddEdge adds a directed edge from source to target with the given data.
    // It panics if either source or target does not exist.
    pub fn add_edge(&mut self, source: NodeIndex, target: NodeIndex, data: E) -> EdgeIndex {
        assert!(self.has_node(source), "graph: source node does not exist");
        assert!(self.has_node(target), "graph: target node does not exist");
        let idx = self.next_edge_index;
        self.edges.insert(idx, Edge { source, target, data });
        self.adjacency.entry(source).or_default().push(idx);
        self.reverse_adjacency.entry(target).or_default().push(idx);
        self.next_edge_index += 1;
        idx
    }

    // go: internal/graph/graph.go:177 Graph.GetEdge
    // GetEdge returns the edge at the given index.
    // The second return value is false if the edge does not exist.
    pub fn get_edge(&self, index: EdgeIndex) -> Option<&Edge<E>> {
        self.edges.get(&index)
    }

    // go: internal/graph/graph.go:183 Graph.HasEdge
    // HasEdge returns true if there is an edge from source to target.
    pub fn has_edge(&self, source: NodeIndex, target: NodeIndex) -> bool {
        for ei in adjacency_list(&self.adjacency, source) {
            // Go reads the zero Edge (target 0) for a missing index.
            if self.edges.get(ei).map_or(0, |edge| edge.target) == target {
                return true;
            }
        }
        false
    }

    // go: internal/graph/graph.go:194 Graph.OutgoingEdges
    // OutgoingEdges returns the edge indices for all outgoing edges from the given node.
    // The returned indices preserve insertion order.
    pub fn outgoing_edges(&self, node_index: NodeIndex) -> Vec<EdgeIndex> {
        adjacency_list(&self.adjacency, node_index).to_vec()
    }

    // go: internal/graph/graph.go:200 Graph.IncomingEdges
    // IncomingEdges returns the edge indices for all incoming edges to the given node.
    // The returned indices preserve insertion order.
    pub fn incoming_edges(&self, node_index: NodeIndex) -> Vec<EdgeIndex> {
        adjacency_list(&self.reverse_adjacency, node_index).to_vec()
    }

    // go: internal/graph/graph.go:206 Graph.UpdateEdge
    // UpdateEdge applies fn to the data of the edge at the given index.
    // If the edge does not exist, this is a no-op.
    //
    // Rust: `fn` reads the data by reference (Go passes a copy).
    pub fn update_edge(&mut self, index: EdgeIndex, f: impl FnOnce(&E) -> E) {
        if let Some(edge) = self.edges.get_mut(&index) {
            edge.data = f(&edge.data);
        }
    }

    // go: internal/graph/graph.go:214 Graph.RemoveEdge
    // RemoveEdge removes the edge at the given index.
    pub fn remove_edge(&mut self, index: EdgeIndex) {
        let Some(edge) = self.edges.get(&index) else {
            return;
        };
        let (source, target) = (edge.source, edge.target);
        remove_from_slice(self.adjacency.entry(source).or_default(), index);
        remove_from_slice(self.reverse_adjacency.entry(target).or_default(), index);
        self.edges.remove(&index);
    }

    // go: internal/graph/graph.go:225 Graph.EdgeCount
    // EdgeCount returns the number of edges in the graph.
    pub fn edge_count(&self) -> i64 {
        self.edges.len() as i64
    }

    // go: internal/graph/graph.go:230 Graph.Nodes
    // Nodes iterates all nodes as (index, data) pairs, sorted by index.
    pub fn nodes(&self) -> impl Iterator<Item = (NodeIndex, &N)> {
        let keys = sorted_keys(&self.nodes);
        keys.into_iter().map(|k| (k, &self.nodes[&k]))
    }

    // go: internal/graph/graph.go:246 Graph.Edges
    // Edges iterates all edges as (index, edge) pairs, sorted by index.
    pub fn edges(&self) -> impl Iterator<Item = (EdgeIndex, &Edge<E>)> {
        let keys = sorted_keys(&self.edges);
        keys.into_iter().map(|k| (k, &self.edges[&k]))
    }

    // go: internal/graph/graph.go:263 Graph.FindNode
    // FindNode returns the index of the first node (in index order) matching the predicate.
    // Returns (0, false) if no node matches.
    pub fn find_node(&self, mut predicate: impl FnMut(&N) -> bool) -> Option<NodeIndex> {
        let keys = sorted_keys(&self.nodes);
        keys.into_iter().find(|&k| predicate(&self.nodes[&k]))
    }

    // go: internal/graph/graph.go:278 Graph.FindNodes
    // FindNodes returns the indices of all nodes matching the predicate, in index order.
    pub fn find_nodes(&self, mut predicate: impl FnMut(&N) -> bool) -> Vec<NodeIndex> {
        let keys = sorted_keys(&self.nodes);
        let mut result = Vec::new();
        for k in keys {
            if predicate(&self.nodes[&k]) {
                result.push(k);
            }
        }
        result
    }

    // go: internal/graph/graph.go:295 Graph.FindEdge
    // FindEdge returns the index of the first edge (in index order) matching the predicate.
    // The predicate receives (edgeData, source, target). Returns (0, false) if no edge matches.
    pub fn find_edge(
        &self,
        mut predicate: impl FnMut(&E, NodeIndex, NodeIndex) -> bool,
    ) -> Option<EdgeIndex> {
        let keys = sorted_keys(&self.edges);
        for k in keys {
            let edge = &self.edges[&k];
            if predicate(&edge.data, edge.source, edge.target) {
                return Some(k);
            }
        }
        None
    }

    // go: internal/graph/graph.go:312 Graph.FindEdges
    // FindEdges returns the indices of all edges matching the predicate, in index order.
    // The predicate receives (edgeData, source, target).
    pub fn find_edges(
        &self,
        mut predicate: impl FnMut(&E, NodeIndex, NodeIndex) -> bool,
    ) -> Vec<EdgeIndex> {
        let keys = sorted_keys(&self.edges);
        let mut result = Vec::new();
        for k in keys {
            let edge = &self.edges[&k];
            if predicate(&edge.data, edge.source, edge.target) {
                result.push(k);
            }
        }
        result
    }

    // go: internal/graph/graph.go:331 Graph.NeighborsDirected
    // NeighborsDirected returns the neighbor node indices reachable via edges in the given direction.
    // For Outgoing, returns target nodes. For Incoming, returns source nodes.
    // Does not deduplicate — if multiple edges connect to the same neighbor, it appears multiple times.
    pub fn neighbors_directed(
        &self,
        node_index: NodeIndex,
        direction: Direction,
    ) -> Vec<NodeIndex> {
        let adj_list = if direction == Incoming {
            adjacency_list(&self.reverse_adjacency, node_index)
        } else {
            adjacency_list(&self.adjacency, node_index)
        };
        let mut result = Vec::with_capacity(adj_list.len());
        for ei in adj_list {
            if let Some(edge) = self.edges.get(ei) {
                if direction == Incoming {
                    result.push(edge.source);
                } else {
                    result.push(edge.target);
                }
            }
        }
        result
    }

    // go: internal/graph/graph.go:354 Graph.Externals
    // Externals returns all nodes with no edges in the given direction, in index order.
    // Externals(Outgoing) yields sink/leaf nodes (no outgoing edges).
    // Externals(Incoming) yields source/root nodes (no incoming edges).
    pub fn externals(&self, direction: Direction) -> impl Iterator<Item = (NodeIndex, &N)> {
        let keys = sorted_keys(&self.nodes);
        keys.into_iter().filter_map(move |k| {
            let adj_list = if direction == Incoming {
                adjacency_list(&self.reverse_adjacency, k)
            } else {
                adjacency_list(&self.adjacency, k)
            };
            if adj_list.is_empty() { Some((k, &self.nodes[&k])) } else { None }
        })
    }

    // go: internal/graph/graph.go:379 Graph.Neighbors
    // Neighbors returns all neighbor node indices (union of outgoing targets and incoming sources),
    // deduplicated and sorted by index for determinism.
    pub fn neighbors(&self, node_index: NodeIndex) -> Vec<NodeIndex> {
        let mut seen = FxHashSet::default();
        for ei in adjacency_list(&self.adjacency, node_index) {
            if let Some(edge) = self.edges.get(ei) {
                seen.insert(edge.target);
            }
        }
        for ei in adjacency_list(&self.reverse_adjacency, node_index) {
            if let Some(edge) = self.edges.get(ei) {
                seen.insert(edge.source);
            }
        }
        let mut result: Vec<NodeIndex> = Vec::with_capacity(seen.len());
        for k in seen {
            result.push(k);
        }
        result.sort_unstable();
        result
    }

    // go: internal/graph/graph.go:400 Graph.Reverse
    // Reverse reverses the direction of all edges in the graph in-place.
    pub fn reverse(&mut self) {
        for edge in self.edges.values_mut() {
            std::mem::swap(&mut edge.source, &mut edge.target);
        }
        std::mem::swap(&mut self.adjacency, &mut self.reverse_adjacency);
    }

    // go: internal/graph/graph.go:410 Graph.FilterNodes
    // FilterNodes removes all nodes that do not satisfy the predicate.
    // Incident edges of removed nodes are also removed.
    //
    // Rust: Go collects in map order; the removals give the same graph in any order.
    pub fn filter_nodes(&mut self, mut predicate: impl FnMut(&N) -> bool) {
        let mut to_remove = Vec::new();
        for (&idx, data) in &self.nodes {
            if !predicate(data) {
                to_remove.push(idx);
            }
        }
        for idx in to_remove {
            self.remove_node(idx);
        }
    }

    // go: internal/graph/graph.go:424 Graph.FilterEdges
    // FilterEdges removes all edges whose data does not satisfy the predicate.
    // All nodes are preserved.
    //
    // Rust: Go collects in map order; the removals give the same graph in any order.
    pub fn filter_edges(&mut self, mut predicate: impl FnMut(&E) -> bool) {
        let mut to_remove = Vec::new();
        for (&idx, edge) in &self.edges {
            if !predicate(&edge.data) {
                to_remove.push(idx);
            }
        }
        for idx in to_remove {
            self.remove_edge(idx);
        }
    }

    // go: internal/graph/graph.go:437 Graph.MapNodes
    // MapNodes applies fn to every node's data in-place.
    pub fn map_nodes(&mut self, mut f: impl FnMut(&N) -> N) {
        for data in self.nodes.values_mut() {
            *data = f(data);
        }
    }

    // go: internal/graph/graph.go:444 Graph.MapEdges
    // MapEdges applies fn to every edge's data in-place.
    pub fn map_edges(&mut self, mut f: impl FnMut(&E) -> E) {
        for edge in self.edges.values_mut() {
            edge.data = f(&edge.data);
        }
    }

    // go: internal/graph/graph.go:475 Graph.sortedNeighborsDirected
    // sortedNeighborsDirected returns neighbor node indices reachable via edges in the given direction,
    // sorted by edge index for deterministic traversal order.
    pub(crate) fn sorted_neighbors_directed(
        &self,
        node_index: NodeIndex,
        direction: Direction,
    ) -> Vec<NodeIndex> {
        let adj_list = if direction == Incoming {
            adjacency_list(&self.reverse_adjacency, node_index)
        } else {
            adjacency_list(&self.adjacency, node_index)
        };
        let mut sorted = adj_list.to_vec();
        sorted.sort_unstable();
        let mut result = Vec::with_capacity(sorted.len());
        for ei in sorted {
            if let Some(edge) = self.edges.get(&ei) {
                if direction == Incoming {
                    result.push(edge.source);
                } else {
                    result.push(edge.target);
                }
            }
        }
        result
    }

    // go: internal/graph/graph.go:500 Graph.resolveTraversalConfig
    // resolveTraversalConfig returns the start nodes and direction for a traversal,
    // applying defaults when the config fields are empty.
    pub(crate) fn resolve_traversal_config(
        &self,
        config: TraversalConfig,
    ) -> (Vec<NodeIndex>, Direction) {
        let direction = config.direction.unwrap_or(Outgoing);
        let mut start = config.start;
        if start.is_empty() {
            for (idx, _) in self.externals(Incoming) {
                start.push(idx);
            }
        }
        (start, direction)
    }

    // go: internal/graph/graph.go:516 Graph.DFS
    // DFS performs an iterative pre-order depth-first search, yielding (index, data) pairs.
    // Neighbors are visited in deterministic edge-index order.
    //
    // Rust: Go's lazy `iter.Seq2` is a lazy iterator; the config is resolved when it is created (the graph is
    // borrowed for its whole life, so nothing can change in between).
    pub fn dfs(&self, config: TraversalConfig) -> impl Iterator<Item = (NodeIndex, &N)> {
        let (start, direction) = self.resolve_traversal_config(config);
        let mut discovered = FxHashSet::with_capacity_and_hasher(self.nodes.len(), FxBuildHasher);
        // Initialize stack with start nodes in reverse order so the first is popped first.
        let mut stack: Vec<NodeIndex> = Vec::with_capacity(start.len());
        for &s in start.iter().rev() {
            stack.push(s);
        }
        std::iter::from_fn(move || {
            while let Some(node) = stack.pop() {
                // Pop
                if discovered.contains(&node) {
                    continue;
                }
                discovered.insert(node);
                // Push neighbors in reverse order so lower-indexed neighbors are visited first
                let neighbors = self.sorted_neighbors_directed(node, direction);
                for &neighbor in neighbors.iter().rev() {
                    if !discovered.contains(&neighbor) {
                        stack.push(neighbor);
                    }
                }
                // Yield pre-order
                // Rust: the neighbors are pushed before the yield; Go pushes them when the consumer asks for the
                // next node, which is the same order (the stack is private to the iteration).
                return Some((node, self.node_data(node)));
            }
            None
        })
    }

    // go: internal/graph/graph.go:556 Graph.DFSPostOrder
    // DFSPostOrder performs an iterative post-order depth-first search, yielding (index, data) pairs.
    // Children are emitted before their parents. Neighbors are visited in deterministic edge-index order.
    pub fn dfs_post_order(&self, config: TraversalConfig) -> impl Iterator<Item = (NodeIndex, &N)> {
        let (start, direction) = self.resolve_traversal_config(config);
        let mut discovered = FxHashSet::with_capacity_and_hasher(self.nodes.len(), FxBuildHasher);

        // Initialize stack with start nodes in reverse order (first start node on top)
        let mut stack: Vec<DfsPostOrderEntry> = Vec::with_capacity(start.len());
        for &s in start.iter().rev() {
            stack.push(DfsPostOrderEntry { node: s, expanded: false });
        }

        std::iter::from_fn(move || {
            while let Some(top) = stack.pop() {
                if discovered.contains(&top.node) {
                    if top.expanded {
                        // Second visit — yield post-order
                        return Some((top.node, self.node_data(top.node)));
                    }
                    continue;
                }

                // First visit — mark discovered, push back with expanded=true, then push children
                discovered.insert(top.node);
                stack.push(DfsPostOrderEntry { node: top.node, expanded: true });

                let neighbors = self.sorted_neighbors_directed(top.node, direction);
                for &neighbor in neighbors.iter().rev() {
                    if !discovered.contains(&neighbor) {
                        stack.push(DfsPostOrderEntry { node: neighbor, expanded: false });
                    }
                }
            }
            None
        })
    }

    // go: internal/graph/graph.go:597 Graph.BFS
    // BFS performs a breadth-first search traversal, yielding (index, data) pairs level by level.
    // Neighbors are visited in deterministic edge-index order.
    pub fn bfs(&self, config: TraversalConfig) -> impl Iterator<Item = (NodeIndex, &N)> {
        let (start, direction) = self.resolve_traversal_config(config);
        let mut discovered = FxHashSet::with_capacity_and_hasher(self.nodes.len(), FxBuildHasher);

        // Initialize queue with start nodes, marking them as discovered
        let mut queue: VecDeque<NodeIndex> = VecDeque::with_capacity(start.len());
        for s in start {
            if discovered.insert(s) {
                queue.push_back(s);
            }
        }

        std::iter::from_fn(move || {
            // Dequeue from front
            let node = queue.pop_front()?;

            // Rust: the neighbors are enqueued before the yield (see `dfs`).
            let neighbors = self.sorted_neighbors_directed(node, direction);
            for neighbor in neighbors {
                if discovered.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }

            Some((node, self.node_data(node)))
        })
    }

    // go: internal/graph/graph.go:633 Graph.Topo
    // Topo performs a topological sort using Kahn's algorithm, yielding (index, data) pairs.
    // Only meaningful for acyclic graphs. For cyclic graphs, nodes involved in cycles are not yielded.
    pub fn topo(&self) -> impl Iterator<Item = (NodeIndex, &N)> {
        // Calculate in-degree for every node
        let mut in_degree: FxHashMap<NodeIndex, i64> =
            FxHashMap::with_capacity_and_hasher(self.nodes.len(), FxBuildHasher);
        for &idx in self.nodes.keys() {
            in_degree.insert(idx, adjacency_list(&self.reverse_adjacency, idx).len() as i64);
        }

        // Initialize queue with all zero-in-degree nodes, sorted by index for determinism
        let keys = sorted_keys(&self.nodes);

        let mut queue: Vec<NodeIndex> = Vec::new();
        for k in keys {
            if in_degree.get(&k).copied().unwrap_or(0) == 0 {
                queue.push(k);
            }
        }

        std::iter::from_fn(move || {
            if queue.is_empty() {
                return None;
            }
            // Dequeue front
            let node = queue.remove(0);

            // For each outgoing neighbor, decrement in-degree
            // Rust: before the yield (see `dfs`).
            let neighbors = self.sorted_neighbors_directed(node, Outgoing);
            for neighbor in neighbors {
                let degree = in_degree.entry(neighbor).or_default();
                *degree -= 1;
                if *degree == 0 {
                    // Insert into queue maintaining sorted order for determinism
                    let (pos, _) = ets_go::slices::binary_search(&queue, &neighbor);
                    queue.insert(pos, neighbor);
                }
            }

            Some((node, &self.nodes[&node]))
        })
    }

    // go: internal/graph/graph.go:680 Graph.IsAcyclic
    // IsAcyclic returns true if the graph contains no cycles.
    // Uses iterative DFS with recursion stack tracking to detect back edges.
    pub fn is_acyclic(&self) -> bool {
        let mut visited = FxHashSet::with_capacity_and_hasher(self.nodes.len(), FxBuildHasher);
        let mut recursion_stack = FxHashSet::default();

        let keys = sorted_keys(&self.nodes);

        for start_node in keys {
            if visited.contains(&start_node) {
                continue;
            }

            let mut stack = vec![IsAcyclicStackEntry { node: start_node, neighbor_idx: 0 }];
            visited.insert(start_node);
            recursion_stack.insert(start_node);

            while let Some(top) = stack.last_mut() {
                let neighbors = self.sorted_neighbors_directed(top.node, Outgoing);

                if top.neighbor_idx < neighbors.len() {
                    let neighbor = neighbors[top.neighbor_idx];
                    top.neighbor_idx += 1;

                    if recursion_stack.contains(&neighbor) {
                        return false;
                    }
                    if visited.insert(neighbor) {
                        recursion_stack.insert(neighbor);
                        stack.push(IsAcyclicStackEntry { node: neighbor, neighbor_idx: 0 });
                    }
                } else {
                    recursion_stack.remove(&top.node);
                    stack.pop();
                }
            }
        }

        true
    }

    // go: internal/graph/graph.go:745 Graph.ToMermaid
    // ToMermaid renders the graph as a Mermaid flowchart diagram string.
    pub fn to_mermaid(&self, options: MermaidOptions<'_, N, E>) -> String {
        let mut direction = options.direction.as_str();
        if direction.is_empty() {
            direction = "TB";
        }
        let node_label = options.node_label;
        let node_shape = options.node_shape.unwrap_or_else(|| {
            Box::new(|_: &N| ("[".to_string(), "]".to_string())) as MermaidShape<'_, N>
        });
        let edge_label = options.edge_label;
        let edge_shape = options.edge_shape.unwrap_or_else(|| {
            Box::new(|_: &E| ("-->".to_string(), String::new())) as MermaidShape<'_, E>
        });

        let mut lines: Vec<String> = Vec::new();
        lines.push(format!("flowchart {direction}"));

        // Nodes in index order
        for (idx, data) in self.nodes() {
            let label = escape_mermaid_label(&node_label(data));
            let (open, close_shape) = node_shape(data);
            lines.push(format!("  {idx}{open}\"{label}\"{close_shape}"));
        }

        // Edges in index order
        for (_, edge) in self.edges() {
            let label = escape_mermaid_label(&edge_label(&edge.data));
            let (open, close_shape) = edge_shape(&edge.data);
            if !label.is_empty() {
                lines.push(format!(
                    "  {} {open}|\"{label}\"|{close_shape} {}",
                    edge.source, edge.target
                ));
            } else {
                lines.push(format!("  {} {open}{close_shape} {}", edge.source, edge.target));
            }
        }

        lines.join("\n")
    }
}

// go: internal/graph/graph.go:149 removeFromSlice
// removeFromSlice removes the first occurrence of val from slice.
//
// Rust: removes in place; Go's `append(slice[:i], slice[i+1:]...)` reuses the backing array the same way.
pub(crate) fn remove_from_slice(slice: &mut Vec<EdgeIndex>, val: EdgeIndex) {
    for (i, &v) in slice.iter().enumerate() {
        if v == val {
            slice.remove(i);
            return;
        }
    }
}

// go: internal/graph/graph.go:453 Graph.Clone
// Clone returns a deep copy of the graph. Node and edge data values are
// shallow-copied, but all internal maps and slices are independently allocated.
impl<N: Clone, E: Clone> Clone for Graph<N, E> {
    fn clone(&self) -> Self {
        let mut c = Graph {
            nodes: FxHashMap::with_capacity_and_hasher(self.nodes.len(), FxBuildHasher),
            edges: FxHashMap::with_capacity_and_hasher(self.edges.len(), FxBuildHasher),
            adjacency: FxHashMap::with_capacity_and_hasher(self.adjacency.len(), FxBuildHasher),
            reverse_adjacency: FxHashMap::with_capacity_and_hasher(
                self.reverse_adjacency.len(),
                FxBuildHasher,
            ),
            next_node_index: self.next_node_index,
            next_edge_index: self.next_edge_index,
        };
        c.nodes.extend(self.nodes.iter().map(|(&k, v)| (k, v.clone())));
        c.edges.extend(self.edges.iter().map(|(&k, v)| (k, v.clone())));
        for (&k, v) in &self.adjacency {
            c.adjacency.insert(k, v.clone());
        }
        for (&k, v) in &self.reverse_adjacency {
            c.reverse_adjacency.insert(k, v.clone());
        }
        c
    }
}

// go: internal/graph/graph.go:549 dfsPostOrderEntry
// dfsPostOrderEntry is used by DFSPostOrder for the two-phase stack approach.
#[derive(Clone, Copy)]
pub(crate) struct DfsPostOrderEntry {
    node: NodeIndex,
    expanded: bool,
}

// go: internal/graph/graph.go:726 escapeMermaidLabel
// escapeMermaidLabel escapes special characters in a Mermaid label.
pub(crate) fn escape_mermaid_label(label: &str) -> String {
    let mut label = label.replace('#', "#35;");
    label = label.replace('"', "#quot;");
    label = label.replace('<', "#lt;");
    label = label.replace('>', "#gt;");
    label = label.replace('&', "#amp;");
    label = label.replace('[', "#91;");
    label = label.replace(']', "#93;");
    label = label.replace('{', "#123;");
    label = label.replace('}', "#125;");
    label = label.replace('(', "#40;");
    label = label.replace(')', "#41;");
    label = label.replace('|', "#124;");
    label = label.replace('\\', "#92;");
    label = label.replace('\n', "<br/>");
    label
}
