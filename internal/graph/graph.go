// Package graph provides a generic directed graph data structure for modeling
// typed relationships between nodes.
package graph

import (
	"fmt"
	"iter"
	"maps"
	"slices"
	"strings"
)

// NodeIndex identifies a node in the graph. Indices are auto-incremented and never reused.
type NodeIndex = int

// EdgeIndex identifies an edge in the graph. Indices are auto-incremented and never reused.
type EdgeIndex = int

// Direction specifies the direction of traversal or query.
type Direction string

const (
	// Outgoing follows edges from source to target.
	Outgoing Direction = "Outgoing"
	// Incoming follows edges from target to source.
	Incoming Direction = "Incoming"
)

// Edge represents a directed edge between two nodes with associated data.
type Edge[E any] struct {
	Source NodeIndex
	Target NodeIndex
	Data   E
}

// TraversalConfig configures traversal algorithms (DFS, BFS, etc.).
// When Start is empty, defaults to all source nodes (nodes with no incoming edges).
// When Direction is empty, defaults to Outgoing.
type TraversalConfig struct {
	Start     []NodeIndex
	Direction Direction
}

// MermaidOptions configures Mermaid flowchart rendering.
type MermaidOptions[N any, E any] struct {
	NodeLabel func(N) string
	EdgeLabel func(E) string
	Direction string
}

// isAcyclicStackEntry is used by IsAcyclic for iterative DFS cycle detection.
type isAcyclicStackEntry struct {
	node        NodeIndex
	neighborIdx int
}

// Graph is a generic directed graph with typed node and edge data.
// It uses adjacency lists backed by maps for O(1) lookups.
type Graph[N any, E any] struct {
	nodes            map[NodeIndex]N
	edges            map[EdgeIndex]Edge[E]
	adjacency        map[NodeIndex][]EdgeIndex
	reverseAdjacency map[NodeIndex][]EdgeIndex
	nextNodeIndex    NodeIndex
	nextEdgeIndex    EdgeIndex
}

// New creates an empty directed graph.
func New[N any, E any]() *Graph[N, E] {
	return &Graph[N, E]{
		nodes:            make(map[NodeIndex]N),
		edges:            make(map[EdgeIndex]Edge[E]),
		adjacency:        make(map[NodeIndex][]EdgeIndex),
		reverseAdjacency: make(map[NodeIndex][]EdgeIndex),
	}
}

// AddNode adds a node with the given data and returns its index.
func (g *Graph[N, E]) AddNode(data N) NodeIndex {
	idx := g.nextNodeIndex
	g.nodes[idx] = data
	g.adjacency[idx] = []EdgeIndex{}
	g.reverseAdjacency[idx] = []EdgeIndex{}
	g.nextNodeIndex++
	return idx
}

// GetNode returns the data for the node at the given index.
// The second return value is false if the node does not exist.
func (g *Graph[N, E]) GetNode(index NodeIndex) (N, bool) {
	data, ok := g.nodes[index]
	return data, ok
}

// HasNode returns true if a node with the given index exists.
func (g *Graph[N, E]) HasNode(index NodeIndex) bool {
	_, ok := g.nodes[index]
	return ok
}

// UpdateNode applies fn to the data of the node at the given index.
// If the node does not exist, this is a no-op.
func (g *Graph[N, E]) UpdateNode(index NodeIndex, fn func(N) N) {
	if data, ok := g.nodes[index]; ok {
		g.nodes[index] = fn(data)
	}
}

// RemoveNode removes the node at the given index and all its incident edges.
func (g *Graph[N, E]) RemoveNode(index NodeIndex) {
	if !g.HasNode(index) {
		return
	}

	// Collect all edge indices to remove (deduplicate since self-loops appear in both lists)
	edgeSet := make(map[EdgeIndex]struct{})
	for _, ei := range g.adjacency[index] {
		edgeSet[ei] = struct{}{}
	}
	for _, ei := range g.reverseAdjacency[index] {
		edgeSet[ei] = struct{}{}
	}

	// Remove each edge from the opposite node's adjacency lists and from edges map
	for ei := range edgeSet {
		edge := g.edges[ei]
		if edge.Source != index {
			g.adjacency[edge.Source] = removeFromSlice(g.adjacency[edge.Source], ei)
		}
		if edge.Target != index {
			g.reverseAdjacency[edge.Target] = removeFromSlice(g.reverseAdjacency[edge.Target], ei)
		}
		delete(g.edges, ei)
	}

	delete(g.nodes, index)
	delete(g.adjacency, index)
	delete(g.reverseAdjacency, index)
}

// NodeCount returns the number of nodes in the graph.
func (g *Graph[N, E]) NodeCount() int {
	return len(g.nodes)
}

// removeFromSlice removes the first occurrence of val from slice.
func removeFromSlice(slice []EdgeIndex, val EdgeIndex) []EdgeIndex {
	for i, v := range slice {
		if v == val {
			return append(slice[:i], slice[i+1:]...)
		}
	}
	return slice
}

// AddEdge adds a directed edge from source to target with the given data.
// It panics if either source or target does not exist.
func (g *Graph[N, E]) AddEdge(source, target NodeIndex, data E) EdgeIndex {
	if !g.HasNode(source) {
		panic("graph: source node does not exist")
	}
	if !g.HasNode(target) {
		panic("graph: target node does not exist")
	}
	idx := g.nextEdgeIndex
	g.edges[idx] = Edge[E]{Source: source, Target: target, Data: data}
	g.adjacency[source] = append(g.adjacency[source], idx)
	g.reverseAdjacency[target] = append(g.reverseAdjacency[target], idx)
	g.nextEdgeIndex++
	return idx
}

// GetEdge returns the edge at the given index.
// The second return value is false if the edge does not exist.
func (g *Graph[N, E]) GetEdge(index EdgeIndex) (Edge[E], bool) {
	edge, ok := g.edges[index]
	return edge, ok
}

// HasEdge returns true if there is an edge from source to target.
func (g *Graph[N, E]) HasEdge(source, target NodeIndex) bool {
	for _, ei := range g.adjacency[source] {
		if g.edges[ei].Target == target {
			return true
		}
	}
	return false
}

// UpdateEdge applies fn to the data of the edge at the given index.
// If the edge does not exist, this is a no-op.
func (g *Graph[N, E]) UpdateEdge(index EdgeIndex, fn func(E) E) {
	if edge, ok := g.edges[index]; ok {
		edge.Data = fn(edge.Data)
		g.edges[index] = edge
	}
}

// RemoveEdge removes the edge at the given index.
func (g *Graph[N, E]) RemoveEdge(index EdgeIndex) {
	edge, ok := g.edges[index]
	if !ok {
		return
	}
	g.adjacency[edge.Source] = removeFromSlice(g.adjacency[edge.Source], index)
	g.reverseAdjacency[edge.Target] = removeFromSlice(g.reverseAdjacency[edge.Target], index)
	delete(g.edges, index)
}

// EdgeCount returns the number of edges in the graph.
func (g *Graph[N, E]) EdgeCount() int {
	return len(g.edges)
}

// Nodes iterates all nodes as (index, data) pairs, sorted by index.
func (g *Graph[N, E]) Nodes() iter.Seq2[NodeIndex, N] {
	return func(yield func(NodeIndex, N) bool) {
		keys := make([]NodeIndex, 0, len(g.nodes))
		for k := range g.nodes {
			keys = append(keys, k)
		}
		slices.Sort(keys)
		for _, k := range keys {
			if !yield(k, g.nodes[k]) {
				return
			}
		}
	}
}

// Edges iterates all edges as (index, edge) pairs, sorted by index.
func (g *Graph[N, E]) Edges() iter.Seq2[EdgeIndex, Edge[E]] {
	return func(yield func(EdgeIndex, Edge[E]) bool) {
		keys := make([]EdgeIndex, 0, len(g.edges))
		for k := range g.edges {
			keys = append(keys, k)
		}
		slices.Sort(keys)
		for _, k := range keys {
			if !yield(k, g.edges[k]) {
				return
			}
		}
	}
}

// FindNode returns the index of the first node (in index order) matching the predicate.
// Returns (0, false) if no node matches.
func (g *Graph[N, E]) FindNode(predicate func(N) bool) (NodeIndex, bool) {
	keys := make([]NodeIndex, 0, len(g.nodes))
	for k := range g.nodes {
		keys = append(keys, k)
	}
	slices.Sort(keys)
	for _, k := range keys {
		if predicate(g.nodes[k]) {
			return k, true
		}
	}
	return 0, false
}

// FindNodes returns the indices of all nodes matching the predicate, in index order.
func (g *Graph[N, E]) FindNodes(predicate func(N) bool) []NodeIndex {
	keys := make([]NodeIndex, 0, len(g.nodes))
	for k := range g.nodes {
		keys = append(keys, k)
	}
	slices.Sort(keys)
	var result []NodeIndex
	for _, k := range keys {
		if predicate(g.nodes[k]) {
			result = append(result, k)
		}
	}
	return result
}

// FindEdge returns the index of the first edge (in index order) matching the predicate.
// The predicate receives (edgeData, source, target). Returns (0, false) if no edge matches.
func (g *Graph[N, E]) FindEdge(predicate func(E, NodeIndex, NodeIndex) bool) (EdgeIndex, bool) {
	keys := make([]EdgeIndex, 0, len(g.edges))
	for k := range g.edges {
		keys = append(keys, k)
	}
	slices.Sort(keys)
	for _, k := range keys {
		edge := g.edges[k]
		if predicate(edge.Data, edge.Source, edge.Target) {
			return k, true
		}
	}
	return 0, false
}

// FindEdges returns the indices of all edges matching the predicate, in index order.
// The predicate receives (edgeData, source, target).
func (g *Graph[N, E]) FindEdges(predicate func(E, NodeIndex, NodeIndex) bool) []EdgeIndex {
	keys := make([]EdgeIndex, 0, len(g.edges))
	for k := range g.edges {
		keys = append(keys, k)
	}
	slices.Sort(keys)
	var result []EdgeIndex
	for _, k := range keys {
		edge := g.edges[k]
		if predicate(edge.Data, edge.Source, edge.Target) {
			result = append(result, k)
		}
	}
	return result
}

// NeighborsDirected returns the neighbor node indices reachable via edges in the given direction.
// For Outgoing, returns target nodes. For Incoming, returns source nodes.
// Does not deduplicate — if multiple edges connect to the same neighbor, it appears multiple times.
func (g *Graph[N, E]) NeighborsDirected(nodeIndex NodeIndex, direction Direction) []NodeIndex {
	var adjList []EdgeIndex
	if direction == Incoming {
		adjList = g.reverseAdjacency[nodeIndex]
	} else {
		adjList = g.adjacency[nodeIndex]
	}
	result := make([]NodeIndex, 0, len(adjList))
	for _, ei := range adjList {
		if edge, ok := g.edges[ei]; ok {
			if direction == Incoming {
				result = append(result, edge.Source)
			} else {
				result = append(result, edge.Target)
			}
		}
	}
	return result
}

// Externals returns all nodes with no edges in the given direction, in index order.
// Externals(Outgoing) yields sink/leaf nodes (no outgoing edges).
// Externals(Incoming) yields source/root nodes (no incoming edges).
func (g *Graph[N, E]) Externals(direction Direction) iter.Seq2[NodeIndex, N] {
	return func(yield func(NodeIndex, N) bool) {
		keys := make([]NodeIndex, 0, len(g.nodes))
		for k := range g.nodes {
			keys = append(keys, k)
		}
		slices.Sort(keys)
		for _, k := range keys {
			var adjList []EdgeIndex
			if direction == Incoming {
				adjList = g.reverseAdjacency[k]
			} else {
				adjList = g.adjacency[k]
			}
			if len(adjList) == 0 {
				if !yield(k, g.nodes[k]) {
					return
				}
			}
		}
	}
}

// Neighbors returns all neighbor node indices (union of outgoing targets and incoming sources),
// deduplicated and sorted by index for determinism.
func (g *Graph[N, E]) Neighbors(nodeIndex NodeIndex) []NodeIndex {
	seen := make(map[NodeIndex]struct{})
	for _, ei := range g.adjacency[nodeIndex] {
		if edge, ok := g.edges[ei]; ok {
			seen[edge.Target] = struct{}{}
		}
	}
	for _, ei := range g.reverseAdjacency[nodeIndex] {
		if edge, ok := g.edges[ei]; ok {
			seen[edge.Source] = struct{}{}
		}
	}
	result := make([]NodeIndex, 0, len(seen))
	for k := range seen {
		result = append(result, k)
	}
	slices.Sort(result)
	return result
}

// Reverse reverses the direction of all edges in the graph in-place.
func (g *Graph[N, E]) Reverse() {
	for idx, edge := range g.edges {
		edge.Source, edge.Target = edge.Target, edge.Source
		g.edges[idx] = edge
	}
	g.adjacency, g.reverseAdjacency = g.reverseAdjacency, g.adjacency
}

// FilterNodes removes all nodes that do not satisfy the predicate.
// Incident edges of removed nodes are also removed.
func (g *Graph[N, E]) FilterNodes(predicate func(N) bool) {
	var toRemove []NodeIndex
	for idx, data := range g.nodes {
		if !predicate(data) {
			toRemove = append(toRemove, idx)
		}
	}
	for _, idx := range toRemove {
		g.RemoveNode(idx)
	}
}

// FilterEdges removes all edges whose data does not satisfy the predicate.
// All nodes are preserved.
func (g *Graph[N, E]) FilterEdges(predicate func(E) bool) {
	var toRemove []EdgeIndex
	for idx, edge := range g.edges {
		if !predicate(edge.Data) {
			toRemove = append(toRemove, idx)
		}
	}
	for _, idx := range toRemove {
		g.RemoveEdge(idx)
	}
}

// MapNodes applies fn to every node's data in-place.
func (g *Graph[N, E]) MapNodes(fn func(N) N) {
	for idx, data := range g.nodes {
		g.nodes[idx] = fn(data)
	}
}

// MapEdges applies fn to every edge's data in-place.
func (g *Graph[N, E]) MapEdges(fn func(E) E) {
	for idx, edge := range g.edges {
		edge.Data = fn(edge.Data)
		g.edges[idx] = edge
	}
}

// Clone returns a deep copy of the graph. Node and edge data values are
// shallow-copied, but all internal maps and slices are independently allocated.
func (g *Graph[N, E]) Clone() *Graph[N, E] {
	c := &Graph[N, E]{
		nodes:            make(map[NodeIndex]N, len(g.nodes)),
		edges:            make(map[EdgeIndex]Edge[E], len(g.edges)),
		adjacency:        make(map[NodeIndex][]EdgeIndex, len(g.adjacency)),
		reverseAdjacency: make(map[NodeIndex][]EdgeIndex, len(g.reverseAdjacency)),
		nextNodeIndex:    g.nextNodeIndex,
		nextEdgeIndex:    g.nextEdgeIndex,
	}
	maps.Copy(c.nodes, g.nodes)
	maps.Copy(c.edges, g.edges)
	for k, v := range g.adjacency {
		c.adjacency[k] = append([]EdgeIndex(nil), v...)
	}
	for k, v := range g.reverseAdjacency {
		c.reverseAdjacency[k] = append([]EdgeIndex(nil), v...)
	}
	return c
}

// sortedNeighborsDirected returns neighbor node indices reachable via edges in the given direction,
// sorted by edge index for deterministic traversal order.
func (g *Graph[N, E]) sortedNeighborsDirected(nodeIndex NodeIndex, direction Direction) []NodeIndex {
	var adjList []EdgeIndex
	if direction == Incoming {
		adjList = g.reverseAdjacency[nodeIndex]
	} else {
		adjList = g.adjacency[nodeIndex]
	}
	sorted := make([]EdgeIndex, len(adjList))
	copy(sorted, adjList)
	slices.Sort(sorted)
	result := make([]NodeIndex, 0, len(sorted))
	for _, ei := range sorted {
		if edge, ok := g.edges[ei]; ok {
			if direction == Incoming {
				result = append(result, edge.Source)
			} else {
				result = append(result, edge.Target)
			}
		}
	}
	return result
}

// resolveTraversalConfig returns the start nodes and direction for a traversal,
// applying defaults when the config fields are empty.
func (g *Graph[N, E]) resolveTraversalConfig(config TraversalConfig) ([]NodeIndex, Direction) {
	direction := config.Direction
	if direction == "" {
		direction = Outgoing
	}
	start := config.Start
	if len(start) == 0 {
		for idx := range g.Externals(Incoming) {
			start = append(start, idx)
		}
	}
	return start, direction
}

// DFS performs an iterative pre-order depth-first search, yielding (index, data) pairs.
// Neighbors are visited in deterministic edge-index order.
func (g *Graph[N, E]) DFS(config TraversalConfig) iter.Seq2[NodeIndex, N] {
	return func(yield func(NodeIndex, N) bool) {
		start, direction := g.resolveTraversalConfig(config)
		discovered := make(map[NodeIndex]struct{}, len(g.nodes))
		// Initialize stack with start nodes in reverse order so the first is popped first.
		stack := make([]NodeIndex, 0, len(start))
		for i := len(start) - 1; i >= 0; i-- {
			stack = append(stack, start[i])
		}
		for len(stack) > 0 {
			// Pop
			node := stack[len(stack)-1]
			stack = stack[:len(stack)-1]
			if _, ok := discovered[node]; ok {
				continue
			}
			discovered[node] = struct{}{}
			// Yield pre-order
			if !yield(node, g.nodes[node]) {
				return
			}
			// Push neighbors in reverse order so lower-indexed neighbors are visited first
			neighbors := g.sortedNeighborsDirected(node, direction)
			for i := len(neighbors) - 1; i >= 0; i-- {
				if _, ok := discovered[neighbors[i]]; !ok {
					stack = append(stack, neighbors[i])
				}
			}
		}
	}
}

// dfsPostOrderEntry is used by DFSPostOrder for the two-phase stack approach.
type dfsPostOrderEntry struct {
	node     NodeIndex
	expanded bool
}

// DFSPostOrder performs an iterative post-order depth-first search, yielding (index, data) pairs.
// Children are emitted before their parents. Neighbors are visited in deterministic edge-index order.
func (g *Graph[N, E]) DFSPostOrder(config TraversalConfig) iter.Seq2[NodeIndex, N] {
	return func(yield func(NodeIndex, N) bool) {
		start, direction := g.resolveTraversalConfig(config)
		discovered := make(map[NodeIndex]struct{}, len(g.nodes))

		// Initialize stack with start nodes in reverse order (first start node on top)
		stack := make([]dfsPostOrderEntry, 0, len(start))
		for i := len(start) - 1; i >= 0; i-- {
			stack = append(stack, dfsPostOrderEntry{node: start[i]})
		}

		for len(stack) > 0 {
			top := stack[len(stack)-1]
			stack = stack[:len(stack)-1]

			if _, ok := discovered[top.node]; ok {
				if top.expanded {
					// Second visit — yield post-order
					if !yield(top.node, g.nodes[top.node]) {
						return
					}
				}
				continue
			}

			// First visit — mark discovered, push back with expanded=true, then push children
			discovered[top.node] = struct{}{}
			stack = append(stack, dfsPostOrderEntry{node: top.node, expanded: true})

			neighbors := g.sortedNeighborsDirected(top.node, direction)
			for i := len(neighbors) - 1; i >= 0; i-- {
				if _, ok := discovered[neighbors[i]]; !ok {
					stack = append(stack, dfsPostOrderEntry{node: neighbors[i]})
				}
			}
		}
	}
}

// BFS performs a breadth-first search traversal, yielding (index, data) pairs level by level.
// Neighbors are visited in deterministic edge-index order.
func (g *Graph[N, E]) BFS(config TraversalConfig) iter.Seq2[NodeIndex, N] {
	return func(yield func(NodeIndex, N) bool) {
		start, direction := g.resolveTraversalConfig(config)
		discovered := make(map[NodeIndex]struct{}, len(g.nodes))

		// Initialize queue with start nodes, marking them as discovered
		queue := make([]NodeIndex, 0, len(start))
		for _, s := range start {
			if _, ok := discovered[s]; !ok {
				discovered[s] = struct{}{}
				queue = append(queue, s)
			}
		}

		for len(queue) > 0 {
			// Dequeue from front
			node := queue[0]
			queue = queue[1:]

			if !yield(node, g.nodes[node]) {
				return
			}

			neighbors := g.sortedNeighborsDirected(node, direction)
			for _, neighbor := range neighbors {
				if _, ok := discovered[neighbor]; !ok {
					discovered[neighbor] = struct{}{}
					queue = append(queue, neighbor)
				}
			}
		}
	}
}

// Topo performs a topological sort using Kahn's algorithm, yielding (index, data) pairs.
// Only meaningful for acyclic graphs. For cyclic graphs, nodes involved in cycles are not yielded.
func (g *Graph[N, E]) Topo() iter.Seq2[NodeIndex, N] {
	return func(yield func(NodeIndex, N) bool) {
		// Calculate in-degree for every node
		inDegree := make(map[NodeIndex]int, len(g.nodes))
		for idx := range g.nodes {
			inDegree[idx] = len(g.reverseAdjacency[idx])
		}

		// Initialize queue with all zero-in-degree nodes, sorted by index for determinism
		keys := make([]NodeIndex, 0, len(g.nodes))
		for k := range g.nodes {
			keys = append(keys, k)
		}
		slices.Sort(keys)

		queue := make([]NodeIndex, 0)
		for _, k := range keys {
			if inDegree[k] == 0 {
				queue = append(queue, k)
			}
		}

		for len(queue) > 0 {
			// Dequeue front
			node := queue[0]
			queue = queue[1:]

			if !yield(node, g.nodes[node]) {
				return
			}

			// For each outgoing neighbor, decrement in-degree
			neighbors := g.sortedNeighborsDirected(node, Outgoing)
			for _, neighbor := range neighbors {
				inDegree[neighbor]--
				if inDegree[neighbor] == 0 {
					// Insert into queue maintaining sorted order for determinism
					pos, _ := slices.BinarySearch(queue, neighbor)
					queue = slices.Insert(queue, pos, neighbor)
				}
			}
		}
	}
}

// IsAcyclic returns true if the graph contains no cycles.
// Uses iterative DFS with recursion stack tracking to detect back edges.
func (g *Graph[N, E]) IsAcyclic() bool {
	visited := make(map[NodeIndex]struct{}, len(g.nodes))
	recursionStack := make(map[NodeIndex]struct{})

	keys := make([]NodeIndex, 0, len(g.nodes))
	for k := range g.nodes {
		keys = append(keys, k)
	}
	slices.Sort(keys)

	for _, startNode := range keys {
		if _, ok := visited[startNode]; ok {
			continue
		}

		stack := []isAcyclicStackEntry{{node: startNode, neighborIdx: 0}}
		visited[startNode] = struct{}{}
		recursionStack[startNode] = struct{}{}

		for len(stack) > 0 {
			top := &stack[len(stack)-1]
			neighbors := g.sortedNeighborsDirected(top.node, Outgoing)

			if top.neighborIdx < len(neighbors) {
				neighbor := neighbors[top.neighborIdx]
				top.neighborIdx++

				if _, inRecStack := recursionStack[neighbor]; inRecStack {
					return false
				}
				if _, vis := visited[neighbor]; !vis {
					visited[neighbor] = struct{}{}
					recursionStack[neighbor] = struct{}{}
					stack = append(stack, isAcyclicStackEntry{node: neighbor, neighborIdx: 0})
				}
			} else {
				delete(recursionStack, top.node)
				stack = stack[:len(stack)-1]
			}
		}
	}

	return true
}

// escapeMermaidLabel escapes special characters in a Mermaid label.
func escapeMermaidLabel(label string) string {
	label = strings.ReplaceAll(label, `\`, `\\`)
	label = strings.ReplaceAll(label, `"`, `\"`)
	label = strings.ReplaceAll(label, `[`, `\[`)
	label = strings.ReplaceAll(label, `]`, `\]`)
	label = strings.ReplaceAll(label, `|`, `\|`)
	label = strings.ReplaceAll(label, "\n", "<br/>")
	return label
}

// ToMermaid renders the graph as a Mermaid flowchart diagram string.
func (g *Graph[N, E]) ToMermaid(options MermaidOptions[N, E]) string {
	direction := options.Direction
	if direction == "" {
		direction = "TB"
	}
	nodeLabel := options.NodeLabel
	if nodeLabel == nil {
		nodeLabel = func(data N) string { return fmt.Sprint(data) }
	}
	edgeLabel := options.EdgeLabel
	if edgeLabel == nil {
		edgeLabel = func(data E) string { return fmt.Sprint(data) }
	}

	var lines []string
	lines = append(lines, "flowchart "+direction)

	// Nodes in index order
	for idx, data := range g.Nodes() {
		label := escapeMermaidLabel(nodeLabel(data))
		lines = append(lines, fmt.Sprintf("  %d[\"%s\"]", idx, label))
	}

	// Edges in index order
	for _, edge := range g.Edges() {
		label := escapeMermaidLabel(edgeLabel(edge.Data))
		if label != "" {
			lines = append(lines, fmt.Sprintf("  %d -->|\"%s\"| %d", edge.Source, label, edge.Target))
		} else {
			lines = append(lines, fmt.Sprintf("  %d --> %d", edge.Source, edge.Target))
		}
	}

	return strings.Join(lines, "\n")
}
