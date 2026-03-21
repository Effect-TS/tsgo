package layergraph

import (
	"sort"

	"github.com/effect-ts/effect-typescript-go/internal/graph"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// walkLeavesMatching performs a BFS from the given start node indices, walking
// outgoing edges. For each node, it checks if any outgoing neighbors match the
// predicate. If matching neighbors exist, it continues walking to those neighbors.
// If no matching neighbors exist and the current node matches the predicate,
// the current node is emitted as a "leaf match". This finds the deepest nodes
// in the graph that match the predicate.
func walkLeavesMatching(
	g *graph.Graph[LayerGraphNodeInfo, LayerGraphEdgeInfo],
	startIndices []graph.NodeIndex,
	predicate func(LayerGraphNodeInfo) bool,
) []LayerGraphNodeInfo {
	var result []LayerGraphNodeInfo
	queue := make([]graph.NodeIndex, 0, len(startIndices))
	queue = append(queue, startIndices...)
	discovered := make(map[graph.NodeIndex]bool)

	for len(queue) > 0 {
		current := queue[0]
		queue = queue[1:]

		if discovered[current] {
			continue
		}
		discovered[current] = true

		neighbors := g.NeighborsDirected(current, graph.Outgoing)
		var matchingNeighbors []graph.NodeIndex
		for _, neighbor := range neighbors {
			neighborData, ok := g.GetNode(neighbor)
			if ok && predicate(neighborData) {
				matchingNeighbors = append(matchingNeighbors, neighbor)
			}
		}

		if len(matchingNeighbors) > 0 {
			queue = append(queue, matchingNeighbors...)
		} else {
			nodeData, ok := g.GetNode(current)
			if ok && predicate(nodeData) {
				result = append(result, nodeData)
			}
		}
	}

	return result
}

// ExtractProvidersAndRequirers extracts summary information about which layers
// provide or require which services. It uses a two-step algorithm:
//
// Step 1: Get root nodes (no incoming edges) to determine WHICH types to report.
// The root node's Layer type reflects the net result of the composition, so
// internally-satisfied services are already excluded.
//
// Step 2: For each type from step 1, use walkLeavesMatching BFS to find the
// deepest leaf nodes that match, giving source location attribution.
func ExtractProvidersAndRequirers(
	c *checker.Checker,
	layerGraph *graph.Graph[LayerGraphNodeInfo, LayerGraphEdgeInfo],
) []ProviderRequirerInfo {
	// Step 1: Get root nodes (externals with no incoming edges).
	var rootNodes []LayerGraphNodeInfo
	var rootNodeIndices []graph.NodeIndex
	for idx, node := range layerGraph.Externals(graph.Incoming) {
		rootNodes = append(rootNodes, node)
		rootNodeIndices = append(rootNodeIndices, idx)
	}

	// Collect unique Provides and Requires types from root nodes.
	providesSet := make(map[*checker.Type]bool)
	requiresSet := make(map[*checker.Type]bool)
	for _, root := range rootNodes {
		for _, t := range root.Provides {
			providesSet[t] = true
		}
		for _, t := range root.Requires {
			requiresSet[t] = true
		}
	}

	var result []ProviderRequirerInfo

	// Step 2: For each type, walk leaves to find attribution.
	walkTypes := func(typeSet map[*checker.Type]bool, kind ProviderRequirerKind) {
		// Sort types alphabetically for deterministic output. This intentionally
		// diverges from the .repos reference implementation which uses non-deterministic
		// checker iteration order, to avoid flaky output from Go's map iteration.
		var types []*checker.Type
		for t := range typeSet {
			types = append(types, t)
		}
		sort.Slice(types, func(i, j int) bool {
			return c.TypeToString(types[i]) < c.TypeToString(types[j])
		})

		for _, layerType := range types {
			predicate := func(node LayerGraphNodeInfo) bool {
				var typeList []*checker.Type
				if kind == ProviderRequirerKindProvided {
					typeList = node.Provides
				} else {
					typeList = node.Requires
				}
				for _, t := range typeList {
					if t == layerType || checker.Checker_isTypeAssignableTo(c, t, layerType) {
						return true
					}
				}
				return false
			}

			leafNodes := walkLeavesMatching(layerGraph, rootNodeIndices, predicate)

			var astNodes []*ast.Node
			var displayNodes []*ast.Node
			for _, node := range leafNodes {
				astNodes = append(astNodes, node.Node)
				displayNodes = append(displayNodes, node.DisplayNode)
			}

			result = append(result, ProviderRequirerInfo{
				Kind:         kind,
				Type:         layerType,
				Nodes:        astNodes,
				DisplayNodes: displayNodes,
			})
		}
	}

	walkTypes(providesSet, ProviderRequirerKindProvided)
	walkTypes(requiresSet, ProviderRequirerKindRequired)

	return result
}
