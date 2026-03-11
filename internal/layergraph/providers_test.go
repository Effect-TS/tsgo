package layergraph

import (
	"testing"

	"github.com/effect-ts/effect-typescript-go/internal/graph"
	"github.com/microsoft/typescript-go/shim/checker"
)

// makeNodeInfo creates a minimal LayerGraphNodeInfo for testing walkLeavesMatching.
// When hasTag is true, Provides gets a non-empty slice so the predicate matches.
func makeNodeInfo(hasTag bool) LayerGraphNodeInfo {
	info := LayerGraphNodeInfo{}
	if hasTag {
		info.Provides = []*checker.Type{nil}
	}
	return info
}

// tagPredicate returns true for nodes where Provides is non-empty.
func tagPredicate(node LayerGraphNodeInfo) bool {
	return len(node.Provides) > 0
}

func TestWalkLeavesMatching(t *testing.T) {
	t.Run("single node that matches", func(t *testing.T) {
		g := graph.New[LayerGraphNodeInfo, LayerGraphEdgeInfo]()
		n0 := g.AddNode(makeNodeInfo(true))

		result := walkLeavesMatching(g, []graph.NodeIndex{n0}, tagPredicate)
		if len(result) != 1 {
			t.Fatalf("expected 1 result, got %d", len(result))
		}
	})

	t.Run("single node that does not match", func(t *testing.T) {
		g := graph.New[LayerGraphNodeInfo, LayerGraphEdgeInfo]()
		n0 := g.AddNode(makeNodeInfo(false))

		result := walkLeavesMatching(g, []graph.NodeIndex{n0}, tagPredicate)
		if len(result) != 0 {
			t.Fatalf("expected 0 results, got %d", len(result))
		}
	})

	t.Run("linear chain, all match, emits deepest", func(t *testing.T) {
		// root(match) -> middle(match) -> leaf(match)
		// Should emit only the leaf (deepest match).
		g := graph.New[LayerGraphNodeInfo, LayerGraphEdgeInfo]()
		root := g.AddNode(makeNodeInfo(true))
		middle := g.AddNode(makeNodeInfo(true))
		leaf := g.AddNode(makeNodeInfo(true))
		g.AddEdge(root, middle, LayerGraphEdgeInfo{})
		g.AddEdge(middle, leaf, LayerGraphEdgeInfo{})

		result := walkLeavesMatching(g, []graph.NodeIndex{root}, tagPredicate)
		if len(result) != 1 {
			t.Fatalf("expected 1 result (deepest leaf), got %d", len(result))
		}
	})

	t.Run("linear chain, only root matches", func(t *testing.T) {
		// root(match) -> middle(no) -> leaf(no)
		// root's outgoing neighbor (middle) doesn't match, so root is emitted.
		g := graph.New[LayerGraphNodeInfo, LayerGraphEdgeInfo]()
		root := g.AddNode(makeNodeInfo(true))
		middle := g.AddNode(makeNodeInfo(false))
		leaf := g.AddNode(makeNodeInfo(false))
		g.AddEdge(root, middle, LayerGraphEdgeInfo{})
		g.AddEdge(middle, leaf, LayerGraphEdgeInfo{})

		result := walkLeavesMatching(g, []graph.NodeIndex{root}, tagPredicate)
		if len(result) != 1 {
			t.Fatalf("expected 1 result, got %d", len(result))
		}
	})

	t.Run("branching graph, multiple leaves match", func(t *testing.T) {
		// root(match) -> left(match), root -> right(match)
		// left and right are both leaf matches (no outgoing matching neighbors).
		g := graph.New[LayerGraphNodeInfo, LayerGraphEdgeInfo]()
		root := g.AddNode(makeNodeInfo(true))
		left := g.AddNode(makeNodeInfo(true))
		right := g.AddNode(makeNodeInfo(true))
		g.AddEdge(root, left, LayerGraphEdgeInfo{})
		g.AddEdge(root, right, LayerGraphEdgeInfo{})

		result := walkLeavesMatching(g, []graph.NodeIndex{root}, tagPredicate)
		if len(result) != 2 {
			t.Fatalf("expected 2 results, got %d", len(result))
		}
	})

	t.Run("no nodes match predicate", func(t *testing.T) {
		g := graph.New[LayerGraphNodeInfo, LayerGraphEdgeInfo]()
		n0 := g.AddNode(makeNodeInfo(false))
		n1 := g.AddNode(makeNodeInfo(false))
		g.AddEdge(n0, n1, LayerGraphEdgeInfo{})

		result := walkLeavesMatching(g, []graph.NodeIndex{n0}, tagPredicate)
		if len(result) != 0 {
			t.Fatalf("expected 0 results, got %d", len(result))
		}
	})

	t.Run("diamond graph, all match, emits only sink", func(t *testing.T) {
		// A(match) -> B(match), A -> C(match), B -> D(match), C -> D(match)
		// D is the deepest leaf match. Should only emit D.
		g := graph.New[LayerGraphNodeInfo, LayerGraphEdgeInfo]()
		a := g.AddNode(makeNodeInfo(true))
		b := g.AddNode(makeNodeInfo(true))
		c := g.AddNode(makeNodeInfo(true))
		d := g.AddNode(makeNodeInfo(true))
		g.AddEdge(a, b, LayerGraphEdgeInfo{})
		g.AddEdge(a, c, LayerGraphEdgeInfo{})
		g.AddEdge(b, d, LayerGraphEdgeInfo{})
		g.AddEdge(c, d, LayerGraphEdgeInfo{})

		result := walkLeavesMatching(g, []graph.NodeIndex{a}, tagPredicate)
		if len(result) != 1 {
			t.Fatalf("expected 1 result (D), got %d", len(result))
		}
	})
}
