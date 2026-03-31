package layergraph

import (
	"slices"

	"github.com/effect-ts/tsgo/internal/graph"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// ConvertOutlineGraphToLayerMagic takes an outline graph and a set of target output types,
// then computes an ordered list of layer nodes annotated with merges/provides flags
// that determine which Layer.* combinator to use (provide, provideMerge, or merge).
func ConvertOutlineGraphToLayerMagic(
	tp *typeparser.TypeParser,
	outlineGraph *graph.Graph[LayerOutlineGraphNodeInfo, struct{}],
	targetOutputTypes []*checker.Type,
) *LayerMagicResult {
	// Step 1: Unroll target output types into individual types
	var outputTypes []*checker.Type
	for _, t := range targetOutputTypes {
		outputTypes = append(outputTypes, tp.UnrollUnionMembers(t)...)
	}

	// Build a set of missing output types for fast lookup
	missingOutputTypes := make(map[*checker.Type]struct{}, len(outputTypes))
	for _, t := range outputTypes {
		missingOutputTypes[t] = struct{}{}
	}

	// Step 2: Clone and reverse the outline graph
	reversedGraph := outlineGraph.Clone()
	reversedGraph.Reverse()

	// Step 3: Find root indices (nodes with no incoming edges in the reversed graph)
	var rootIndices []graph.NodeIndex
	for idx := range reversedGraph.Externals(graph.Incoming) {
		rootIndices = append(rootIndices, idx)
	}

	// Sort roots by provides count descending, then requires count descending
	slices.SortFunc(rootIndices, func(a, b graph.NodeIndex) int {
		return compareNodesByOrder(reversedGraph, a, b)
	})

	// Step 4: Run custom DFS post-order with ordering on the reversed graph
	allNodes := dfsPostOrderWithOrder(reversedGraph, rootIndices)

	// Step 5: Process each visited node
	var result []LayerMagicNode
	for _, nodeInfo := range allNodes {
		if !ast.IsExpression(nodeInfo.Node) {
			continue
		}

		// Determine if this node should merge (provides a type still in missingOutputTypes)
		shouldMerge := false
		for _, t := range nodeInfo.ActualProvides {
			if _, ok := missingOutputTypes[t]; ok {
				shouldMerge = true
				break
			}
		}

		// If merging, remove actual provides from missing output types
		if shouldMerge {
			for _, t := range nodeInfo.ActualProvides {
				delete(missingOutputTypes, t)
			}
		}

		result = append(result, LayerMagicNode{
			Merges:   shouldMerge,
			Provides: true,
			Node:     nodeInfo.Node,
		})
	}

	// Step 6: Collect remaining missing output types
	var remaining []*checker.Type
	for _, t := range outputTypes {
		if _, ok := missingOutputTypes[t]; ok {
			remaining = append(remaining, t)
		}
	}

	return &LayerMagicResult{
		Nodes:              result,
		MissingOutputTypes: remaining,
	}
}

// dfsPostOrderWithOrder performs an iterative post-order DFS with custom node ordering.
// Neighbors are sorted by provides count descending, then requires count descending.
func dfsPostOrderWithOrder(
	g *graph.Graph[LayerOutlineGraphNodeInfo, struct{}],
	start []graph.NodeIndex,
) []LayerOutlineGraphNodeInfo {
	type entry struct {
		node     graph.NodeIndex
		expanded bool
	}

	discovered := make(map[graph.NodeIndex]struct{})
	var result []LayerOutlineGraphNodeInfo

	// Initialize stack with start nodes in reverse order (first start node on top)
	stack := make([]entry, 0, len(start))
	for i := len(start) - 1; i >= 0; i-- {
		stack = append(stack, entry{node: start[i]})
	}

	for len(stack) > 0 {
		top := stack[len(stack)-1]
		stack = stack[:len(stack)-1]

		if _, ok := discovered[top.node]; ok {
			if top.expanded {
				// Second visit — emit post-order
				if nodeData, exists := g.GetNode(top.node); exists {
					result = append(result, nodeData)
				}
			}
			continue
		}

		// First visit — mark discovered, push back with expanded=true, then push children
		discovered[top.node] = struct{}{}
		stack = append(stack, entry{node: top.node, expanded: true})

		// Get neighbors in outgoing direction and sort by custom order
		neighbors := neighborsOutgoingSorted(g, top.node)
		for i := len(neighbors) - 1; i >= 0; i-- {
			if _, ok := discovered[neighbors[i]]; !ok {
				stack = append(stack, entry{node: neighbors[i]})
			}
		}
	}

	return result
}

// neighborsOutgoingSorted returns outgoing neighbor indices sorted by the layer ordering:
// provides count descending, then requires count descending.
func neighborsOutgoingSorted(
	g *graph.Graph[LayerOutlineGraphNodeInfo, struct{}],
	nodeIndex graph.NodeIndex,
) []graph.NodeIndex {
	neighbors := g.NeighborsDirected(nodeIndex, graph.Outgoing)
	slices.SortFunc(neighbors, func(a, b graph.NodeIndex) int {
		return compareNodesByOrder(g, a, b)
	})
	return neighbors
}

// compareNodesByOrder compares two nodes by provides count descending, then requires count descending.
func compareNodesByOrder(
	g *graph.Graph[LayerOutlineGraphNodeInfo, struct{}],
	a, b graph.NodeIndex,
) int {
	nodeA, _ := g.GetNode(a)
	nodeB, _ := g.GetNode(b)

	// Sort by provides count descending (reverse order)
	if lenDiff := len(nodeB.Provides) - len(nodeA.Provides); lenDiff != 0 {
		return lenDiff
	}
	// Tiebreak by requires count descending (reverse order)
	return len(nodeB.Requires) - len(nodeA.Requires)
}
