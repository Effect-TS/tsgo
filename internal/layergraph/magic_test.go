package layergraph

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/graph"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

func TestCompareNodesByOrder(t *testing.T) {
	t.Parallel()
	g := graph.New[LayerOutlineGraphNodeInfo, struct{}]()

	// Node A: 3 provides, 1 requires
	idxA := g.AddNode(LayerOutlineGraphNodeInfo{
		Provides: make([]*dummyType, 3),
		Requires: make([]*dummyType, 1),
	})
	// Node B: 1 provides, 2 requires
	idxB := g.AddNode(LayerOutlineGraphNodeInfo{
		Provides: make([]*dummyType, 1),
		Requires: make([]*dummyType, 2),
	})
	// Node C: 3 provides, 3 requires
	idxC := g.AddNode(LayerOutlineGraphNodeInfo{
		Provides: make([]*dummyType, 3),
		Requires: make([]*dummyType, 3),
	})

	// A (3 provides) should come before B (1 provides) — compare returns negative
	if cmp := compareNodesByOrder(g, idxA, idxB); cmp >= 0 {
		t.Errorf("expected A < B (more provides first), got %d", cmp)
	}

	// B (1 provides) should come after A (3 provides) — compare returns positive
	if cmp := compareNodesByOrder(g, idxB, idxA); cmp <= 0 {
		t.Errorf("expected B > A, got %d", cmp)
	}

	// A (3 provides, 1 requires) vs C (3 provides, 3 requires) — tiebreak by requires
	if cmp := compareNodesByOrder(g, idxA, idxC); cmp <= 0 {
		t.Errorf("expected A > C (C has more requires, so C first), got %d", cmp)
	}
}

func TestDFSPostOrderWithOrder(t *testing.T) {
	t.Parallel()
	// Build a small graph:
	//   A (3 provides) -> B (1 provides)
	//   A -> C (2 provides)
	//   B -> D (0 provides)
	g := graph.New[LayerOutlineGraphNodeInfo, struct{}]()

	nodeA := makeOutlineNode(3, 0)
	nodeB := makeOutlineNode(1, 0)
	nodeC := makeOutlineNode(2, 0)
	nodeD := makeOutlineNode(0, 0)

	idxA := g.AddNode(nodeA)
	idxB := g.AddNode(nodeB)
	idxC := g.AddNode(nodeC)
	idxD := g.AddNode(nodeD)

	g.AddEdge(idxA, idxB, struct{}{})
	g.AddEdge(idxA, idxC, struct{}{})
	g.AddEdge(idxB, idxD, struct{}{})

	result := dfsPostOrderWithOrder(g, []graph.NodeIndex{idxA})

	// Post-order: children before parents
	// A's neighbors sorted by provides desc: C (2) before B (1)
	// Visiting C first (post-order): C has no children -> emit C
	// Then B: B has child D -> emit D, then B
	// Finally A
	// Expected order: C, D, B, A
	if len(result) != 4 {
		t.Fatalf("expected 4 nodes, got %d", len(result))
	}

	expectedProvides := []int{2, 0, 1, 3}
	for i, expected := range expectedProvides {
		if len(result[i].Provides) != expected {
			t.Errorf("result[%d] expected %d provides, got %d", i, expected, len(result[i].Provides))
		}
	}
}

func TestDFSPostOrderWithOrderMultipleRoots(t *testing.T) {
	t.Parallel()
	g := graph.New[LayerOutlineGraphNodeInfo, struct{}]()

	nodeA := makeOutlineNode(3, 0)
	nodeB := makeOutlineNode(1, 0)

	idxA := g.AddNode(nodeA)
	idxB := g.AddNode(nodeB)

	// Two disconnected roots, sorted by provides desc: A (3) before B (1)
	result := dfsPostOrderWithOrder(g, []graph.NodeIndex{idxA, idxB})

	if len(result) != 2 {
		t.Fatalf("expected 2 nodes, got %d", len(result))
	}
	// Post-order with A first in stack: A emitted, then B
	// But roots are sorted by provides desc in the caller, so A (3) is first root
	// Post-order of single nodes: just emit them
	// Start order is reversed on stack, so B pushed first, A on top
	// A popped first -> emitted first, then B
	if len(result[0].Provides) != 3 {
		t.Errorf("expected first node to have 3 provides, got %d", len(result[0].Provides))
	}
	if len(result[1].Provides) != 1 {
		t.Errorf("expected second node to have 1 provides, got %d", len(result[1].Provides))
	}
}

func TestNeighborsOutgoingSorted(t *testing.T) {
	t.Parallel()
	g := graph.New[LayerOutlineGraphNodeInfo, struct{}]()

	nodeA := makeOutlineNode(0, 0)
	nodeB := makeOutlineNode(1, 0)
	nodeC := makeOutlineNode(3, 0)
	nodeD := makeOutlineNode(2, 0)

	idxA := g.AddNode(nodeA)
	idxB := g.AddNode(nodeB)
	idxC := g.AddNode(nodeC)
	idxD := g.AddNode(nodeD)

	g.AddEdge(idxA, idxB, struct{}{})
	g.AddEdge(idxA, idxC, struct{}{})
	g.AddEdge(idxA, idxD, struct{}{})

	neighbors := neighborsOutgoingSorted(g, idxA)

	// Should be sorted by provides desc: C (3), D (2), B (1)
	if len(neighbors) != 3 {
		t.Fatalf("expected 3 neighbors, got %d", len(neighbors))
	}
	if neighbors[0] != idxC {
		t.Errorf("expected first neighbor to be C (3 provides), got index %d", neighbors[0])
	}
	if neighbors[1] != idxD {
		t.Errorf("expected second neighbor to be D (2 provides), got index %d", neighbors[1])
	}
	if neighbors[2] != idxB {
		t.Errorf("expected third neighbor to be B (1 provides), got index %d", neighbors[2])
	}
}

// dummyType is a type alias used only for creating slices of the right length in tests.
// Since we only need len(Provides) and len(Requires) for ordering, actual type values don't matter.
type dummyType = checker.Type

// makeOutlineNode creates a LayerOutlineGraphNodeInfo for testing with the given
// number of provides and requires (using nil pointers, sufficient for ordering tests).
func makeOutlineNode(numProvides, numRequires int) LayerOutlineGraphNodeInfo {
	node := &ast.Node{Kind: ast.KindCallExpression}
	return LayerOutlineGraphNodeInfo{
		Node:           node,
		DisplayNode:    node,
		Provides:       make([]*dummyType, numProvides),
		ActualProvides: make([]*dummyType, numProvides),
		Requires:       make([]*dummyType, numRequires),
	}
}
