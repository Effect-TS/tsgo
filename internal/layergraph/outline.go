package layergraph

import (
	"sort"

	"github.com/effect-ts/effect-typescript-go/internal/graph"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// ExtractOutlineGraph builds a simplified dependency graph from the full layer graph.
// It keeps only leaf nodes (deduplicated by symbol) and connects them based on
// type compatibility: if node A requires a service that node B provides, add an edge.
func ExtractOutlineGraph(
	c *checker.Checker,
	layerGraph *graph.Graph[LayerGraphNodeInfo, LayerGraphEdgeInfo],
) *graph.Graph[LayerOutlineGraphNodeInfo, struct{}] {
	g := graph.New[LayerOutlineGraphNodeInfo, struct{}]()

	// Track providers: providedType → list of outline node indices that provide it.
	providers := make(map[*checker.Type][]graph.NodeIndex)
	// Track seen symbols for deduplication.
	knownSymbols := make(map[*ast.Symbol]struct{})

	// Step 1: Get leaf nodes (nodes with no outgoing edges) and deduplicate by symbol.
	var dedupedLeafNodes []LayerGraphNodeInfo
	for _, leafNode := range layerGraph.Externals(graph.Outgoing) {
		sym := c.GetSymbolAtLocation(leafNode.Node)
		if sym == nil {
			dedupedLeafNodes = append(dedupedLeafNodes, leafNode)
		} else if _, seen := knownSymbols[sym]; !seen {
			dedupedLeafNodes = append(dedupedLeafNodes, leafNode)
			knownSymbols[sym] = struct{}{}
		}
	}

	// Step 2: Create outline nodes and build the provider map.
	for _, leafNode := range dedupedLeafNodes {
		nodeIndex := g.AddNode(LayerOutlineGraphNodeInfo{
			Node:           leafNode.Node,
			DisplayNode:    leafNode.DisplayNode,
			Provides:       leafNode.Provides,
			ActualProvides: leafNode.ActualProvides,
			Requires:       leafNode.Requires,
		})
		for _, providedType := range leafNode.ActualProvides {
			providers[providedType] = append(providers[providedType], nodeIndex)
		}
	}

	// Sort provider types alphabetically for deterministic edge ordering. This
	// intentionally diverges from the .repos reference implementation which uses
	// non-deterministic checker iteration order, to avoid flaky output from Go's
	// map iteration.
	var sortedProviderTypes []*checker.Type
	for t := range providers {
		sortedProviderTypes = append(sortedProviderTypes, t)
	}
	sort.Slice(sortedProviderTypes, func(i, j int) bool {
		return c.TypeToString(sortedProviderTypes[i]) < c.TypeToString(sortedProviderTypes[j])
	})

	// Step 3: Connect requires to providers based on type assignability.
	for nodeIndex, nodeInfo := range g.Nodes() {
		for _, requiredType := range nodeInfo.Requires {
			for _, providedType := range sortedProviderTypes {
				providerNodeIndices := providers[providedType]
				if requiredType == providedType || checker.Checker_isTypeAssignableTo(c, requiredType, providedType) {
					for _, providerNodeIndex := range providerNodeIndices {
						if !g.HasEdge(nodeIndex, providerNodeIndex) {
							g.AddEdge(nodeIndex, providerNodeIndex, struct{}{})
						}
					}
				}
			}
		}
	}

	return g
}
