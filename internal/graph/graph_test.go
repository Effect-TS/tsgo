package graph

import (
	"iter"
	"slices"
	"strings"
	"testing"
)

// buildDiamondGraph creates: A→B, A→C, B→D, C→D
func buildDiamondGraph() (*Graph[string, string], [4]NodeIndex) {
	g := New[string, string]()
	a := g.AddNode("A")
	b := g.AddNode("B")
	c := g.AddNode("C")
	d := g.AddNode("D")
	g.AddEdge(a, b, "ab")
	g.AddEdge(a, c, "ac")
	g.AddEdge(b, d, "bd")
	g.AddEdge(c, d, "cd")
	return g, [4]NodeIndex{a, b, c, d}
}

// buildChainGraph creates: A→B→C
func buildChainGraph() (*Graph[string, string], [3]NodeIndex) {
	g := New[string, string]()
	a := g.AddNode("A")
	b := g.AddNode("B")
	c := g.AddNode("C")
	g.AddEdge(a, b, "ab")
	g.AddEdge(b, c, "bc")
	return g, [3]NodeIndex{a, b, c}
}

// collectValues collects the values from an iter.Seq2 into a slice.
func collectValues[K comparable, V any](seq iter.Seq2[K, V]) []V {
	var result []V
	for _, v := range seq {
		result = append(result, v)
	}
	return result
}

// collectIndices collects the indices from an iter.Seq2 into a slice.
func collectIndices[V any](seq iter.Seq2[int, V]) []int {
	var result []int
	for idx := range seq {
		result = append(result, idx)
	}
	return result
}

func TestNodeOperations(t *testing.T) {
	t.Parallel()
	t.Run("Add", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		c := g.AddNode("C")
		if a != 0 || b != 1 || c != 2 {
			t.Errorf("expected sequential indices 0,1,2 but got %d,%d,%d", a, b, c)
		}
		for _, idx := range []NodeIndex{a, b, c} {
			if _, ok := g.GetNode(idx); !ok {
				t.Errorf("node %d not found after Add", idx)
			}
		}
	})

	t.Run("GetExisting", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("hello")
		data, ok := g.GetNode(a)
		if !ok {
			t.Fatal("expected node to exist")
		}
		if data != "hello" {
			t.Errorf("expected data %q, got %q", "hello", data)
		}
	})

	t.Run("GetNonExisting", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		data, ok := g.GetNode(999)
		if ok {
			t.Fatal("expected node not to exist")
		}
		if data != "" {
			t.Errorf("expected zero value, got %q", data)
		}
	})

	t.Run("Has", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		if !g.HasNode(a) {
			t.Error("expected HasNode to return true for existing node")
		}
		if g.HasNode(999) {
			t.Error("expected HasNode to return false for non-existing node")
		}
	})

	t.Run("Update", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("old")
		g.UpdateNode(a, func(_ string) string { return "new" })
		data, _ := g.GetNode(a)
		if data != "new" {
			t.Errorf("expected %q after update, got %q", "new", data)
		}
	})

	t.Run("UpdateNonExisting", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		// Should not panic
		g.UpdateNode(999, func(_ string) string { return "x" })
	})

	t.Run("Remove", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		g.AddEdge(a, b, "ab")
		g.RemoveNode(a)
		if g.HasNode(a) {
			t.Error("expected node to be removed")
		}
		if g.EdgeCount() != 0 {
			t.Errorf("expected 0 edges after removing node, got %d", g.EdgeCount())
		}
	})

	t.Run("RemoveSelfLoop", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		g.AddEdge(a, a, "self")
		if g.EdgeCount() != 1 {
			t.Fatalf("expected 1 edge, got %d", g.EdgeCount())
		}
		g.RemoveNode(a)
		if g.EdgeCount() != 0 {
			t.Errorf("expected 0 edges after removing self-loop node, got %d", g.EdgeCount())
		}
	})

	t.Run("Count", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		if g.NodeCount() != 0 {
			t.Errorf("expected 0, got %d", g.NodeCount())
		}
		a := g.AddNode("A")
		g.AddNode("B")
		if g.NodeCount() != 2 {
			t.Errorf("expected 2, got %d", g.NodeCount())
		}
		g.RemoveNode(a)
		if g.NodeCount() != 1 {
			t.Errorf("expected 1 after remove, got %d", g.NodeCount())
		}
	})
}

func TestEdgeOperations(t *testing.T) {
	t.Parallel()
	t.Run("Add", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		ei := g.AddEdge(a, b, "ab")
		if !g.HasEdge(a, b) {
			t.Error("expected HasEdge to return true")
		}
		edge, ok := g.GetEdge(ei)
		if !ok {
			t.Fatal("expected edge to exist")
		}
		if edge.Data != "ab" {
			t.Errorf("expected edge data %q, got %q", "ab", edge.Data)
		}
	})

	t.Run("PanicsOnInvalidSource", func(t *testing.T) {
		t.Parallel()
		defer func() {
			if r := recover(); r == nil {
				t.Errorf("expected panic for invalid source node")
			}
		}()
		g := New[string, string]()
		g.AddNode("A")
		g.AddEdge(999, 0, "x")
	})

	t.Run("PanicsOnInvalidTarget", func(t *testing.T) {
		t.Parallel()
		defer func() {
			if r := recover(); r == nil {
				t.Errorf("expected panic for invalid target node")
			}
		}()
		g := New[string, string]()
		g.AddNode("A")
		g.AddEdge(0, 999, "x")
	})

	t.Run("GetExisting", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		ei := g.AddEdge(a, b, "ab")
		edge, ok := g.GetEdge(ei)
		if !ok {
			t.Fatal("expected edge to exist")
		}
		if edge.Source != a || edge.Target != b || edge.Data != "ab" {
			t.Errorf("unexpected edge: %+v", edge)
		}
	})

	t.Run("GetNonExisting", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		_, ok := g.GetEdge(999)
		if ok {
			t.Error("expected edge not to exist")
		}
	})

	t.Run("Has", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		g.AddEdge(a, b, "ab")
		if !g.HasEdge(a, b) {
			t.Error("expected HasEdge true for existing edge")
		}
		if g.HasEdge(b, a) {
			t.Error("expected HasEdge false for non-existing edge")
		}
	})

	t.Run("Update", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		ei := g.AddEdge(a, b, "old")
		g.UpdateEdge(ei, func(_ string) string { return "new" })
		edge, _ := g.GetEdge(ei)
		if edge.Data != "new" {
			t.Errorf("expected %q, got %q", "new", edge.Data)
		}
	})

	t.Run("UpdateNonExisting", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		// Should not panic
		g.UpdateEdge(999, func(_ string) string { return "x" })
	})

	t.Run("Remove", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		ei := g.AddEdge(a, b, "ab")
		g.RemoveEdge(ei)
		if g.HasEdge(a, b) {
			t.Error("expected edge to be removed")
		}
		if !g.HasNode(a) || !g.HasNode(b) {
			t.Error("expected nodes to be preserved")
		}
	})

	t.Run("Count", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		if g.EdgeCount() != 0 {
			t.Errorf("expected 0, got %d", g.EdgeCount())
		}
		ei := g.AddEdge(a, b, "ab")
		if g.EdgeCount() != 1 {
			t.Errorf("expected 1, got %d", g.EdgeCount())
		}
		g.RemoveEdge(ei)
		if g.EdgeCount() != 0 {
			t.Errorf("expected 0 after remove, got %d", g.EdgeCount())
		}
	})

	t.Run("MultipleSamePair", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		g.AddEdge(a, b, "first")
		g.AddEdge(a, b, "second")
		if g.EdgeCount() != 2 {
			t.Errorf("expected 2 edges, got %d", g.EdgeCount())
		}
	})
}

func TestIteration(t *testing.T) {
	t.Parallel()
	t.Run("Nodes", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		values := collectValues(g.Nodes())
		expected := []string{"A", "B", "C", "D"}
		if !slices.Equal(values, expected) {
			t.Errorf("expected %v, got %v", expected, values)
		}
	})

	t.Run("Edges", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		edges := collectValues(g.Edges())
		if len(edges) != 4 {
			t.Fatalf("expected 4 edges, got %d", len(edges))
		}
		// Edges should be in insertion order (sorted by index)
		if edges[0].Source != n[0] || edges[0].Target != n[1] || edges[0].Data != "ab" {
			t.Errorf("unexpected first edge: %+v", edges[0])
		}
		if edges[3].Source != n[2] || edges[3].Target != n[3] || edges[3].Data != "cd" {
			t.Errorf("unexpected last edge: %+v", edges[3])
		}
	})

	t.Run("Deterministic", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		first := collectValues(g.Nodes())
		second := collectValues(g.Nodes())
		if !slices.Equal(first, second) {
			t.Errorf("iteration not deterministic: %v vs %v", first, second)
		}
	})
}

func TestQuerying(t *testing.T) {
	t.Parallel()
	t.Run("FindNodeMatching", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		idx, ok := g.FindNode(func(s string) bool { return s == "C" })
		if !ok {
			t.Fatal("expected to find node")
		}
		if idx != n[2] {
			t.Errorf("expected index %d, got %d", n[2], idx)
		}
	})

	t.Run("FindNodeNone", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		_, ok := g.FindNode(func(s string) bool { return s == "Z" })
		if ok {
			t.Error("expected not to find node")
		}
	})

	t.Run("FindNodesMultiple", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		indices := g.FindNodes(func(s string) bool { return s == "B" || s == "C" })
		expected := []NodeIndex{n[1], n[2]}
		if !slices.Equal(indices, expected) {
			t.Errorf("expected %v, got %v", expected, indices)
		}
	})

	t.Run("FindNodesNone", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		indices := g.FindNodes(func(s string) bool { return s == "Z" })
		if len(indices) != 0 {
			t.Errorf("expected empty, got %v", indices)
		}
	})

	t.Run("FindEdgeMatching", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		idx, ok := g.FindEdge(func(data string, _, _ NodeIndex) bool { return data == "bd" })
		if !ok {
			t.Fatal("expected to find edge")
		}
		edge, _ := g.GetEdge(idx)
		if edge.Data != "bd" {
			t.Errorf("expected data %q, got %q", "bd", edge.Data)
		}
	})

	t.Run("FindEdgeNone", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		_, ok := g.FindEdge(func(data string, _, _ NodeIndex) bool { return data == "zz" })
		if ok {
			t.Error("expected not to find edge")
		}
	})

	t.Run("FindEdgesMultiple", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		indices := g.FindEdges(func(data string, _, _ NodeIndex) bool {
			return data == "ab" || data == "cd"
		})
		if len(indices) != 2 {
			t.Errorf("expected 2 edges, got %d", len(indices))
		}
	})

	t.Run("FindEdgesNone", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		indices := g.FindEdges(func(_ string, _, _ NodeIndex) bool { return false })
		if len(indices) != 0 {
			t.Errorf("expected empty, got %v", indices)
		}
	})
}

func TestNeighborhood(t *testing.T) {
	t.Parallel()
	t.Run("Neighbors", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		// B's neighbors: A (incoming) and D (outgoing)
		neighbors := g.Neighbors(n[1])
		expected := []NodeIndex{n[0], n[3]}
		if !slices.Equal(neighbors, expected) {
			t.Errorf("expected %v, got %v", expected, neighbors)
		}
	})

	t.Run("NeighborsDirectedOutgoing", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		// A's outgoing: B, C
		neighbors := g.NeighborsDirected(n[0], Outgoing)
		expected := []NodeIndex{n[1], n[2]}
		if !slices.Equal(neighbors, expected) {
			t.Errorf("expected %v, got %v", expected, neighbors)
		}
	})

	t.Run("NeighborsDirectedIncoming", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		// D's incoming: B, C
		neighbors := g.NeighborsDirected(n[3], Incoming)
		expected := []NodeIndex{n[1], n[2]}
		if !slices.Equal(neighbors, expected) {
			t.Errorf("expected %v, got %v", expected, neighbors)
		}
	})

	t.Run("OutgoingEdges", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		edges := g.OutgoingEdges(n[0])
		expected := []EdgeIndex{0, 1}
		if !slices.Equal(edges, expected) {
			t.Errorf("expected %v, got %v", expected, edges)
		}
	})

	t.Run("IncomingEdges", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		edges := g.IncomingEdges(n[3])
		expected := []EdgeIndex{2, 3}
		if !slices.Equal(edges, expected) {
			t.Errorf("expected %v, got %v", expected, edges)
		}
	})
}

func TestExternals(t *testing.T) {
	t.Parallel()
	t.Run("Sinks", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		sinks := collectValues(g.Externals(Outgoing))
		expected := []string{"D"}
		if !slices.Equal(sinks, expected) {
			t.Errorf("expected %v, got %v", expected, sinks)
		}
		indices := collectIndices(g.Externals(Outgoing))
		if !slices.Equal(indices, []int{n[3]}) {
			t.Errorf("expected sink index %v, got %v", []int{n[3]}, indices)
		}
	})

	t.Run("Sources", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		sources := collectValues(g.Externals(Incoming))
		expected := []string{"A"}
		if !slices.Equal(sources, expected) {
			t.Errorf("expected %v, got %v", expected, sources)
		}
		indices := collectIndices(g.Externals(Incoming))
		if !slices.Equal(indices, []int{n[0]}) {
			t.Errorf("expected source index %v, got %v", []int{n[0]}, indices)
		}
	})
}

func TestTransformations(t *testing.T) {
	t.Parallel()
	t.Run("Reverse", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		g.Reverse()
		// After reverse, edge a→b becomes b→a
		if !g.HasEdge(n[1], n[0]) {
			t.Error("expected edge B→A after reverse")
		}
		if g.HasEdge(n[0], n[1]) {
			t.Error("expected no edge A→B after reverse")
		}
	})

	t.Run("FilterNodes", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		// Keep only A and B
		g.FilterNodes(func(s string) bool { return s == "A" || s == "B" })
		if g.NodeCount() != 2 {
			t.Errorf("expected 2 nodes, got %d", g.NodeCount())
		}
		// Only edge A→B should remain (edges to C and D removed)
		if g.EdgeCount() != 1 {
			t.Errorf("expected 1 edge, got %d", g.EdgeCount())
		}
	})

	t.Run("FilterEdges", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		g.FilterEdges(func(s string) bool { return s == "ab" || s == "cd" })
		if g.EdgeCount() != 2 {
			t.Errorf("expected 2 edges, got %d", g.EdgeCount())
		}
		// All nodes should be preserved
		if g.NodeCount() != 4 {
			t.Errorf("expected 4 nodes, got %d", g.NodeCount())
		}
	})

	t.Run("MapNodes", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		g.MapNodes(strings.ToUpper)
		data, _ := g.GetNode(n[0])
		if data != "A" {
			t.Errorf("expected %q, got %q", "A", data)
		}
		// Already uppercase, let's use lowercase first
		g2 := New[string, string]()
		idx := g2.AddNode("hello")
		g2.MapNodes(strings.ToUpper)
		d, _ := g2.GetNode(idx)
		if d != "HELLO" {
			t.Errorf("expected %q, got %q", "HELLO", d)
		}
	})

	t.Run("MapEdges", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		g.MapEdges(strings.ToUpper)
		edges := collectValues(g.Edges())
		if edges[0].Data != "AB" {
			t.Errorf("expected %q, got %q", "AB", edges[0].Data)
		}
	})
}

func TestClone(t *testing.T) {
	t.Parallel()
	t.Run("MatchesOriginal", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		c := g.Clone()
		if c.NodeCount() != g.NodeCount() {
			t.Errorf("node count mismatch: %d vs %d", c.NodeCount(), g.NodeCount())
		}
		if c.EdgeCount() != g.EdgeCount() {
			t.Errorf("edge count mismatch: %d vs %d", c.EdgeCount(), g.EdgeCount())
		}
		origNodes := collectValues(g.Nodes())
		cloneNodes := collectValues(c.Nodes())
		if !slices.Equal(origNodes, cloneNodes) {
			t.Errorf("node data mismatch: %v vs %v", origNodes, cloneNodes)
		}
	})

	t.Run("MutateClone", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		c := g.Clone()
		c.AddNode("E")
		if g.NodeCount() != 4 {
			t.Errorf("original should be unaffected, got %d nodes", g.NodeCount())
		}
	})

	t.Run("MutateOriginal", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		c := g.Clone()
		g.AddNode("E")
		if c.NodeCount() != 4 {
			t.Errorf("clone should be unaffected, got %d nodes", c.NodeCount())
		}
	})
}

func TestDFS(t *testing.T) {
	t.Parallel()
	t.Run("Chain", func(t *testing.T) {
		t.Parallel()
		g, _ := buildChainGraph()
		result := collectValues(g.DFS(TraversalConfig{}))
		expected := []string{"A", "B", "C"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})

	t.Run("Diamond", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		result := collectValues(g.DFS(TraversalConfig{}))
		// A first, then B (lower edge-index), then D (B's child), then C
		expected := []string{"A", "B", "D", "C"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})

	t.Run("CustomStart", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		result := collectValues(g.DFS(TraversalConfig{Start: []NodeIndex{n[1]}}))
		expected := []string{"B", "D"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})

	t.Run("IncomingDirection", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		result := collectValues(g.DFS(TraversalConfig{Start: []NodeIndex{n[3]}, Direction: Incoming}))
		// From D following incoming edges: D, then B (lower edge-index bd<cd), then A, then C
		expected := []string{"D", "B", "A", "C"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})

	t.Run("Disconnected", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		c := g.AddNode("C")
		d := g.AddNode("D")
		g.AddEdge(a, b, "ab")
		g.AddEdge(c, d, "cd")
		result := collectValues(g.DFS(TraversalConfig{}))
		expected := []string{"A", "B", "C", "D"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})

	t.Run("EarlyTermination", func(t *testing.T) {
		t.Parallel()
		g, _ := buildChainGraph()
		var result []string
		for _, v := range g.DFS(TraversalConfig{}) {
			result = append(result, v)
			break
		}
		if len(result) != 1 {
			t.Errorf("expected 1 element, got %d", len(result))
		}
		if result[0] != "A" {
			t.Errorf("expected %q, got %q", "A", result[0])
		}
	})
}

func TestDFSPostOrder(t *testing.T) {
	t.Parallel()
	t.Run("Chain", func(t *testing.T) {
		t.Parallel()
		g, _ := buildChainGraph()
		result := collectValues(g.DFSPostOrder(TraversalConfig{}))
		expected := []string{"C", "B", "A"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})

	t.Run("Diamond", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		result := collectValues(g.DFSPostOrder(TraversalConfig{}))
		// D first (deepest via B path), then B, then C (D already visited), then A
		expected := []string{"D", "B", "C", "A"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})

	t.Run("CustomStart", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		result := collectValues(g.DFSPostOrder(TraversalConfig{Start: []NodeIndex{n[1]}}))
		expected := []string{"D", "B"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})
}

func TestBFS(t *testing.T) {
	t.Parallel()
	t.Run("Chain", func(t *testing.T) {
		t.Parallel()
		g, _ := buildChainGraph()
		result := collectValues(g.BFS(TraversalConfig{}))
		expected := []string{"A", "B", "C"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})

	t.Run("Diamond", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		result := collectValues(g.BFS(TraversalConfig{}))
		// A (level 0), B and C (level 1), D (level 2)
		expected := []string{"A", "B", "C", "D"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})

	t.Run("CustomStart", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		result := collectValues(g.BFS(TraversalConfig{Start: []NodeIndex{n[1]}}))
		expected := []string{"B", "D"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})

	t.Run("IncomingDirection", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		result := collectValues(g.BFS(TraversalConfig{Start: []NodeIndex{n[3]}, Direction: Incoming}))
		// From D: D, then B and C (incoming neighbors), then A
		expected := []string{"D", "B", "C", "A"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})
}

func TestTopo(t *testing.T) {
	t.Parallel()
	t.Run("Chain", func(t *testing.T) {
		t.Parallel()
		g, _ := buildChainGraph()
		result := collectValues(g.Topo())
		expected := []string{"A", "B", "C"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})

	t.Run("Diamond", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		result := collectValues(g.Topo())
		// A first, then B and C (in index order), then D
		expected := []string{"A", "B", "C", "D"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})

	t.Run("MultipleRoots", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		r1 := g.AddNode("R1")
		r2 := g.AddNode("R2")
		tt := g.AddNode("T")
		g.AddEdge(r1, tt, "r1t")
		g.AddEdge(r2, tt, "r2t")
		result := collectValues(g.Topo())
		expected := []string{"R1", "R2", "T"}
		if !slices.Equal(result, expected) {
			t.Errorf("expected %v, got %v", expected, result)
		}
	})

	t.Run("Cyclic", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		c := g.AddNode("C")
		g.AddEdge(a, b, "ab")
		g.AddEdge(b, c, "bc")
		g.AddEdge(c, a, "ca")
		result := collectValues(g.Topo())
		if len(result) != 0 {
			t.Errorf("expected empty result for cyclic graph, got %v", result)
		}
	})
}

func TestIsAcyclic(t *testing.T) {
	t.Parallel()
	t.Run("DAG", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		if !g.IsAcyclic() {
			t.Error("expected diamond graph to be acyclic")
		}
	})

	t.Run("WithCycle", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		c := g.AddNode("C")
		g.AddEdge(a, b, "ab")
		g.AddEdge(b, c, "bc")
		g.AddEdge(c, a, "ca")
		if g.IsAcyclic() {
			t.Error("expected cyclic graph to not be acyclic")
		}
	})

	t.Run("EmptyGraph", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		if !g.IsAcyclic() {
			t.Error("expected empty graph to be acyclic")
		}
	})
}

func TestToMermaid(t *testing.T) {
	t.Parallel()
	t.Run("Simple", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		g.AddEdge(a, b, "ab")
		result := g.ToMermaid(MermaidOptions[string, string]{})
		expected := `flowchart TB
  0["A"]
  1["B"]
  0 -->|"ab"| 1`
		if result != expected {
			t.Errorf("expected:\n%s\ngot:\n%s", expected, result)
		}
	})

	t.Run("CustomLabels", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		g.AddEdge(a, b, "ab")
		result := g.ToMermaid(MermaidOptions[string, string]{
			NodeLabel: func(s string) string { return "node:" + s },
			EdgeLabel: func(s string) string { return "edge:" + s },
		})
		if !strings.Contains(result, "node:A") {
			t.Error("expected custom node label prefix")
		}
		if !strings.Contains(result, "edge:ab") {
			t.Error("expected custom edge label prefix")
		}
	})

	t.Run("Direction", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		result := g.ToMermaid(MermaidOptions[string, string]{Direction: "LR"})
		if !strings.HasPrefix(result, "flowchart LR") {
			t.Errorf("expected flowchart LR, got %q", result)
		}
	})

	t.Run("Escaping", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		g.AddNode(`#a\b"c<d>[e]{f}(g)|h&i` + "\n" + "j")
		result := g.ToMermaid(MermaidOptions[string, string]{})
		if !strings.Contains(result, `#35;`) {
			t.Error("expected escaped hash entity")
		}
		if !strings.Contains(result, `#quot;`) {
			t.Error("expected escaped quote entity")
		}
		if !strings.Contains(result, `#lt;`) {
			t.Error("expected escaped less-than entity")
		}
		if !strings.Contains(result, `#gt;`) {
			t.Error("expected escaped greater-than entity")
		}
		if !strings.Contains(result, `#amp;`) {
			t.Error("expected escaped ampersand entity")
		}
		if !strings.Contains(result, `#91;`) {
			t.Error("expected escaped open bracket entity")
		}
		if !strings.Contains(result, `#93;`) {
			t.Error("expected escaped close bracket entity")
		}
		if !strings.Contains(result, `#123;`) {
			t.Error("expected escaped open brace entity")
		}
		if !strings.Contains(result, `#125;`) {
			t.Error("expected escaped close brace entity")
		}
		if !strings.Contains(result, `#40;`) {
			t.Error("expected escaped open parenthesis entity")
		}
		if !strings.Contains(result, `#41;`) {
			t.Error("expected escaped close parenthesis entity")
		}
		if !strings.Contains(result, `#124;`) {
			t.Error("expected escaped pipe entity")
		}
		if !strings.Contains(result, `#92;`) {
			t.Error("expected escaped backslash entity")
		}
		if !strings.Contains(result, "<br/>") {
			t.Error("expected escaped newline as <br/>")
		}
	})

	t.Run("EmptyGraph", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		result := g.ToMermaid(MermaidOptions[string, string]{})
		if result != "flowchart TB" {
			t.Errorf("expected %q, got %q", "flowchart TB", result)
		}
	})

	t.Run("EmptyEdgeLabel", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		b := g.AddNode("B")
		g.AddEdge(a, b, "ab")
		result := g.ToMermaid(MermaidOptions[string, string]{
			EdgeLabel: func(string) string { return "" },
		})
		if !strings.Contains(result, "-->") {
			t.Error("expected --> in output")
		}
		if strings.Contains(result, "-->|") {
			t.Error("expected no label syntax for empty edge label")
		}
	})
}

func TestEdgeCases(t *testing.T) {
	t.Parallel()
	t.Run("EmptyGraph", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		if g.NodeCount() != 0 {
			t.Errorf("expected 0 nodes, got %d", g.NodeCount())
		}
		if g.EdgeCount() != 0 {
			t.Errorf("expected 0 edges, got %d", g.EdgeCount())
		}
		nodes := collectValues(g.Nodes())
		if nodes != nil {
			t.Errorf("expected nil, got %v", nodes)
		}
		if !g.IsAcyclic() {
			t.Error("expected empty graph to be acyclic")
		}
	})

	t.Run("SelfLoop", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		g.AddEdge(a, a, "self")
		if !g.HasEdge(a, a) {
			t.Error("expected self-loop edge")
		}
		g.RemoveNode(a)
		if g.EdgeCount() != 0 {
			t.Errorf("expected 0 edges after removing self-loop node, got %d", g.EdgeCount())
		}
	})

	t.Run("RemoveLastNode", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		a := g.AddNode("A")
		g.RemoveNode(a)
		if g.NodeCount() != 0 {
			t.Errorf("expected 0 nodes, got %d", g.NodeCount())
		}
	})

	t.Run("TraversalEmptyGraph", func(t *testing.T) {
		t.Parallel()
		g := New[string, string]()
		dfs := collectValues(g.DFS(TraversalConfig{}))
		if dfs != nil {
			t.Errorf("expected nil DFS result, got %v", dfs)
		}
		bfs := collectValues(g.BFS(TraversalConfig{}))
		if bfs != nil {
			t.Errorf("expected nil BFS result, got %v", bfs)
		}
		post := collectValues(g.DFSPostOrder(TraversalConfig{}))
		if post != nil {
			t.Errorf("expected nil DFSPostOrder result, got %v", post)
		}
		topo := collectValues(g.Topo())
		if topo != nil {
			t.Errorf("expected nil Topo result, got %v", topo)
		}
	})
}
