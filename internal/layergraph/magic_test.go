package layergraph

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/graph"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func TestCompareNodesByOrder(t *testing.T) {
	t.Parallel()
	g := graph.New[LayerOutlineGraphNodeInfo, struct{}]()
	ordered := []graph.NodeIndex{
		g.AddNode(makeOutlineNode(3, 3)),
		g.AddNode(makeOutlineNode(3, 1)),
		g.AddNode(makeOutlineNode(1, 2)),
		g.AddNode(makeOutlineNode(3, 0)),
		g.AddNode(makeOutlineNode(1, 0)),
		g.AddNode(makeOutlineNode(0, 0)),
		g.AddNode(makeOutlineNode(0, 1)),
	}
	for i := range len(ordered) - 1 {
		if cmp := compareNodesByOrder(g, ordered[i], ordered[i+1]); cmp >= 0 {
			t.Errorf("expected node %d to sort before node %d, got %d", i, i+1, cmp)
		}
		if cmp := compareNodesByOrder(g, ordered[i+1], ordered[i]); cmp <= 0 {
			t.Errorf("expected node %d to sort after node %d, got %d", i+1, i, cmp)
		}
	}
}

func TestDFSPostOrderWithOrder(t *testing.T) {
	t.Parallel()
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

func TestDFSPostOrderWithOrderKeepsRootOrder(t *testing.T) {
	t.Parallel()
	g := graph.New[LayerOutlineGraphNodeInfo, struct{}]()

	nodeA := makeOutlineNode(3, 0)
	nodeB := makeOutlineNode(1, 0)

	idxA := g.AddNode(nodeA)
	idxB := g.AddNode(nodeB)

	result := dfsPostOrderWithOrder(g, []graph.NodeIndex{idxB, idxA})

	if len(result) != 2 {
		t.Fatalf("expected 2 nodes, got %d", len(result))
	}
	if len(result[0].Provides) != 1 || len(result[1].Provides) != 3 {
		t.Errorf("expected roots in the given order (1 then 3 provides), got %d then %d", len(result[0].Provides), len(result[1].Provides))
	}
}

func makeOutlineNode(numProvides, numRequires int) LayerOutlineGraphNodeInfo {
	node := &ast.Node{Kind: ast.KindCallExpression}
	return LayerOutlineGraphNodeInfo{
		Node:           node,
		DisplayNode:    node,
		Provides:       make([]*checker.Type, numProvides),
		ActualProvides: make([]*checker.Type, numProvides),
		Requires:       make([]*checker.Type, numRequires),
	}
}
