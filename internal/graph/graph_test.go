package graph

import (
	"iter"
	"slices"
	"strings"
	"testing"
)

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

func buildChainGraph() (*Graph[string, string], [3]NodeIndex) {
	g := New[string, string]()
	a := g.AddNode("A")
	b := g.AddNode("B")
	c := g.AddNode("C")
	g.AddEdge(a, b, "ab")
	g.AddEdge(b, c, "bc")
	return g, [3]NodeIndex{a, b, c}
}

func buildCycleGraph() *Graph[string, string] {
	g, n := buildChainGraph()
	g.AddEdge(n[2], n[0], "ca")
	return g
}

func collectValues[K comparable, V any](seq iter.Seq2[K, V]) []V {
	var result []V
	for _, v := range seq {
		result = append(result, v)
	}
	return result
}

func collectIndices[V any](seq iter.Seq2[int, V]) []int {
	var result []int
	for idx := range seq {
		result = append(result, idx)
	}
	return result
}

func TestNodeOperations(t *testing.T) {
	t.Parallel()
	g := New[string, string]()
	a := g.AddNode("A")
	b := g.AddNode("B")
	if a != 0 || b != 1 {
		t.Fatalf("expected sequential indices 0,1 but got %d,%d", a, b)
	}
	if data, ok := g.GetNode(a); !ok || data != "A" || !g.HasNode(a) {
		t.Fatalf("GetNode(a) = %q, %v", data, ok)
	}
	if data, ok := g.GetNode(999); ok || data != "" || g.HasNode(999) {
		t.Fatalf("GetNode(999) = %q, %v; want zero value, false", data, ok)
	}
	g.UpdateNode(a, func(string) string { return "new" })
	g.UpdateNode(999, func(string) string { return "x" })
	if data, _ := g.GetNode(a); data != "new" || g.NodeCount() != 2 {
		t.Fatalf("after update: data %q, count %d", data, g.NodeCount())
	}
	g.AddEdge(a, b, "ab")
	g.AddEdge(a, a, "self")
	g.RemoveNode(a)
	if g.HasNode(a) || g.NodeCount() != 1 || g.EdgeCount() != 0 {
		t.Fatalf("after remove: has %v, nodes %d, edges %d", g.HasNode(a), g.NodeCount(), g.EdgeCount())
	}
	g.RemoveNode(b)
	if g.NodeCount() != 0 {
		t.Fatalf("expected 0 nodes, got %d", g.NodeCount())
	}
}

func TestEdgeOperations(t *testing.T) {
	t.Parallel()
	g := New[string, string]()
	a := g.AddNode("A")
	b := g.AddNode("B")
	ei := g.AddEdge(a, b, "old")
	if edge, ok := g.GetEdge(ei); !ok || edge.Source != a || edge.Target != b || edge.Data != "old" {
		t.Fatalf("GetEdge = %+v, %v", edge, ok)
	}
	if !g.HasEdge(a, b) || g.HasEdge(b, a) {
		t.Fatal("HasEdge must follow edge direction")
	}
	if _, ok := g.GetEdge(999); ok {
		t.Fatal("expected edge 999 not to exist")
	}
	g.UpdateEdge(ei, func(string) string { return "new" })
	g.UpdateEdge(999, func(string) string { return "x" })
	if edge, _ := g.GetEdge(ei); edge.Data != "new" {
		t.Fatalf("expected updated edge data, got %q", edge.Data)
	}
	g.AddEdge(a, b, "second")
	if g.EdgeCount() != 2 {
		t.Fatalf("expected parallel edges to be kept, got %d", g.EdgeCount())
	}
	g.RemoveEdge(ei)
	if g.EdgeCount() != 1 || !g.HasNode(a) || !g.HasNode(b) {
		t.Fatalf("RemoveEdge: edges %d, nodes kept %v", g.EdgeCount(), g.HasNode(a) && g.HasNode(b))
	}
	for name, add := range map[string]func(){
		"invalid source": func() { g.AddEdge(999, a, "x") },
		"invalid target": func() { g.AddEdge(a, 999, "x") },
	} {
		func() {
			defer func() {
				if recover() == nil {
					t.Errorf("expected panic for %s", name)
				}
			}()
			add()
		}()
	}
}

func TestIterationAndQuerying(t *testing.T) {
	t.Parallel()
	g, n := buildDiamondGraph()
	if values := collectValues(g.Nodes()); !slices.Equal(values, []string{"A", "B", "C", "D"}) {
		t.Errorf("Nodes() = %v", values)
	}
	var edgeData []string
	for _, edge := range g.Edges() {
		edgeData = append(edgeData, edge.Data)
	}
	if !slices.Equal(edgeData, []string{"ab", "ac", "bd", "cd"}) {
		t.Errorf("Edges() = %v", edgeData)
	}
	if idx, ok := g.FindNode(func(s string) bool { return s == "C" }); !ok || idx != n[2] {
		t.Errorf("FindNode(C) = %d, %v", idx, ok)
	}
	if _, ok := g.FindNode(func(s string) bool { return s == "Z" }); ok {
		t.Error("FindNode(Z) found a node")
	}
	if got := g.FindNodes(func(s string) bool { return s == "B" || s == "C" }); !slices.Equal(got, []NodeIndex{n[1], n[2]}) {
		t.Errorf("FindNodes(B|C) = %v", got)
	}
	if got := g.FindNodes(func(string) bool { return false }); len(got) != 0 {
		t.Errorf("FindNodes(none) = %v", got)
	}
	if idx, ok := g.FindEdge(func(data string, _, _ NodeIndex) bool { return data == "bd" }); !ok || idx != 2 {
		t.Errorf("FindEdge(bd) = %d, %v", idx, ok)
	}
	if _, ok := g.FindEdge(func(string, NodeIndex, NodeIndex) bool { return false }); ok {
		t.Error("FindEdge(none) found an edge")
	}
	if got := g.FindEdges(func(data string, _, _ NodeIndex) bool { return data == "ab" || data == "cd" }); !slices.Equal(got, []EdgeIndex{0, 3}) {
		t.Errorf("FindEdges(ab|cd) = %v", got)
	}
	if got := g.FindEdges(func(string, NodeIndex, NodeIndex) bool { return false }); len(got) != 0 {
		t.Errorf("FindEdges(none) = %v", got)
	}
}

func TestNeighborhood(t *testing.T) {
	t.Parallel()
	g, n := buildDiamondGraph()
	for _, tt := range []struct {
		name     string
		got      []int
		expected []int
	}{
		{"Neighbors", g.Neighbors(n[1]), []NodeIndex{n[0], n[3]}},
		{"NeighborsDirectedOutgoing", g.NeighborsDirected(n[0], Outgoing), []NodeIndex{n[1], n[2]}},
		{"NeighborsDirectedIncoming", g.NeighborsDirected(n[3], Incoming), []NodeIndex{n[1], n[2]}},
		{"OutgoingEdges", g.OutgoingEdges(n[0]), []EdgeIndex{0, 1}},
		{"IncomingEdges", g.IncomingEdges(n[3]), []EdgeIndex{2, 3}},
		{"Sinks", collectIndices(g.Externals(Outgoing)), []NodeIndex{n[3]}},
		{"Sources", collectIndices(g.Externals(Incoming)), []NodeIndex{n[0]}},
	} {
		if !slices.Equal(tt.got, tt.expected) {
			t.Errorf("%s: expected %v, got %v", tt.name, tt.expected, tt.got)
		}
	}
	if sinks := collectValues(g.Externals(Outgoing)); !slices.Equal(sinks, []string{"D"}) {
		t.Errorf("sink values = %v", sinks)
	}
}

func TestTransformations(t *testing.T) {
	t.Parallel()
	t.Run("Reverse", func(t *testing.T) {
		t.Parallel()
		g, n := buildDiamondGraph()
		g.Reverse()
		if !g.HasEdge(n[1], n[0]) || g.HasEdge(n[0], n[1]) {
			t.Error("expected edge A→B to become B→A")
		}
	})

	t.Run("FilterNodes", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		g.FilterNodes(func(s string) bool { return s == "A" || s == "B" })
		if g.NodeCount() != 2 || g.EdgeCount() != 1 {
			t.Errorf("expected 2 nodes and 1 edge, got %d and %d", g.NodeCount(), g.EdgeCount())
		}
	})

	t.Run("FilterEdges", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		g.FilterEdges(func(s string) bool { return s == "ab" || s == "cd" })
		if g.NodeCount() != 4 || g.EdgeCount() != 2 {
			t.Errorf("expected 4 nodes and 2 edges, got %d and %d", g.NodeCount(), g.EdgeCount())
		}
	})

	t.Run("MapNodesAndEdges", func(t *testing.T) {
		t.Parallel()
		g, _ := buildChainGraph()
		g.MapNodes(strings.ToLower)
		g.MapEdges(strings.ToUpper)
		if nodes := collectValues(g.Nodes()); !slices.Equal(nodes, []string{"a", "b", "c"}) {
			t.Errorf("mapped nodes = %v", nodes)
		}
		if edges := collectValues(g.Edges()); edges[0].Data != "AB" || edges[1].Data != "BC" {
			t.Errorf("mapped edges = %+v", edges)
		}
	})

	t.Run("CloneIsIndependent", func(t *testing.T) {
		t.Parallel()
		g, _ := buildDiamondGraph()
		c := g.Clone()
		if !slices.Equal(collectValues(c.Nodes()), collectValues(g.Nodes())) || !slices.Equal(collectValues(c.Edges()), collectValues(g.Edges())) {
			t.Fatal("clone does not match original")
		}
		c.AddNode("E")
		g.RemoveEdge(0)
		if g.NodeCount() != 4 || c.EdgeCount() != 4 {
			t.Errorf("clone shares state: original nodes %d, clone edges %d", g.NodeCount(), c.EdgeCount())
		}
	})
}

func TestTraversalOrder(t *testing.T) {
	t.Parallel()
	diamond, d := buildDiamondGraph()
	chain, _ := buildChainGraph()
	disconnected := New[string, string]()
	for _, name := range []string{"A", "B", "C", "D"} {
		disconnected.AddNode(name)
	}
	disconnected.AddEdge(0, 1, "ab")
	disconnected.AddEdge(2, 3, "cd")
	multipleRoots := New[string, string]()
	r1 := multipleRoots.AddNode("R1")
	r2 := multipleRoots.AddNode("R2")
	target := multipleRoots.AddNode("T")
	multipleRoots.AddEdge(r1, target, "r1t")
	multipleRoots.AddEdge(r2, target, "r2t")
	empty := New[string, string]()
	fromB := TraversalConfig{Start: []NodeIndex{d[1]}}
	fromDIncoming := TraversalConfig{Start: []NodeIndex{d[3]}, Direction: Incoming}

	for _, tt := range []struct {
		name     string
		seq      iter.Seq2[NodeIndex, string]
		expected []string
	}{
		{"DFS/Chain", chain.DFS(TraversalConfig{}), []string{"A", "B", "C"}},
		{"DFS/Diamond", diamond.DFS(TraversalConfig{}), []string{"A", "B", "D", "C"}},
		{"DFS/CustomStart", diamond.DFS(fromB), []string{"B", "D"}},
		{"DFS/IncomingDirection", diamond.DFS(fromDIncoming), []string{"D", "B", "A", "C"}},
		{"DFS/Disconnected", disconnected.DFS(TraversalConfig{}), []string{"A", "B", "C", "D"}},
		{"DFS/Empty", empty.DFS(TraversalConfig{}), nil},
		{"DFSPostOrder/Chain", chain.DFSPostOrder(TraversalConfig{}), []string{"C", "B", "A"}},
		{"DFSPostOrder/Diamond", diamond.DFSPostOrder(TraversalConfig{}), []string{"D", "B", "C", "A"}},
		{"DFSPostOrder/CustomStart", diamond.DFSPostOrder(fromB), []string{"D", "B"}},
		{"DFSPostOrder/Empty", empty.DFSPostOrder(TraversalConfig{}), nil},
		{"BFS/Chain", chain.BFS(TraversalConfig{}), []string{"A", "B", "C"}},
		{"BFS/Diamond", diamond.BFS(TraversalConfig{}), []string{"A", "B", "C", "D"}},
		{"BFS/CustomStart", diamond.BFS(fromB), []string{"B", "D"}},
		{"BFS/IncomingDirection", diamond.BFS(fromDIncoming), []string{"D", "B", "C", "A"}},
		{"BFS/Empty", empty.BFS(TraversalConfig{}), nil},
		{"Topo/Chain", chain.Topo(), []string{"A", "B", "C"}},
		{"Topo/Diamond", diamond.Topo(), []string{"A", "B", "C", "D"}},
		{"Topo/MultipleRoots", multipleRoots.Topo(), []string{"R1", "R2", "T"}},
		{"Topo/Cyclic", buildCycleGraph().Topo(), nil},
		{"Topo/Empty", empty.Topo(), nil},
	} {
		if got := collectValues(tt.seq); !slices.Equal(got, tt.expected) {
			t.Errorf("%s: expected %v, got %v", tt.name, tt.expected, got)
		}
	}

	var visited []string
	for _, v := range chain.DFS(TraversalConfig{}) {
		visited = append(visited, v)
		break
	}
	if !slices.Equal(visited, []string{"A"}) {
		t.Errorf("early termination visited %v", visited)
	}
}

func TestIsAcyclic(t *testing.T) {
	t.Parallel()
	diamond, _ := buildDiamondGraph()
	if !diamond.IsAcyclic() || !New[string, string]().IsAcyclic() {
		t.Error("expected diamond and empty graphs to be acyclic")
	}
	if buildCycleGraph().IsAcyclic() {
		t.Error("expected cyclic graph to not be acyclic")
	}
}

func TestToMermaid(t *testing.T) {
	t.Parallel()
	chain := func() *Graph[string, string] {
		g := New[string, string]()
		g.AddEdge(g.AddNode("A"), g.AddNode("B"), "ab")
		return g
	}
	escaped := New[string, string]()
	escaped.AddNode(`#a\b"c<d>[e]{f}(g)|h&i` + "\n" + "j")

	for _, tt := range []struct {
		name     string
		graph    *Graph[string, string]
		options  MermaidOptions[string, string]
		expected string
	}{
		{"Default", chain(), MermaidOptions[string, string]{}, "flowchart TB\n  0[\"A\"]\n  1[\"B\"]\n  0 -->|\"ab\"| 1"},
		{"Empty", New[string, string](), MermaidOptions[string, string]{Direction: "LR"}, "flowchart LR"},
		{"CustomLabels", chain(), MermaidOptions[string, string]{
			NodeLabel: func(s string) string { return "node:" + s },
			EdgeLabel: func(s string) string { return "edge:" + s },
		}, "flowchart TB\n  0[\"node:A\"]\n  1[\"node:B\"]\n  0 -->|\"edge:ab\"| 1"},
		{"CustomShapes", chain(), MermaidOptions[string, string]{
			NodeShape: func(string) (string, string) { return "([", "])" },
			EdgeShape: func(string) (string, string) { return "-.", ".->" },
		}, "flowchart TB\n  0([\"A\"])\n  1([\"B\"])\n  0 -.|\"ab\"|.-> 1"},
		{"EmptyEdgeLabel", chain(), MermaidOptions[string, string]{
			EdgeLabel: func(string) string { return "" },
		}, "flowchart TB\n  0[\"A\"]\n  1[\"B\"]\n  0 --> 1"},
		{"Escaping", escaped, MermaidOptions[string, string]{}, "flowchart TB\n  0[\"#35;a#92;b#quot;c#lt;d#gt;#91;e#93;#123;f#125;#40;g#41;#124;h#amp;i<br/>j\"]"},
	} {
		if got := tt.graph.ToMermaid(tt.options); got != tt.expected {
			t.Errorf("%s: expected:\n%s\ngot:\n%s", tt.name, tt.expected, got)
		}
	}
}
