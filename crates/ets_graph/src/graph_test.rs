use std::panic::{AssertUnwindSafe, catch_unwind};

use ets_go::sprintf;

use crate::*;

// go test: internal/graph/graph_test.go:11 buildDiamondGraph 6bec62f935998830
// buildDiamondGraph creates: A→B, A→C, B→D, C→D
fn build_diamond_graph() -> (Graph<String, String>, [NodeIndex; 4]) {
    let mut g = new::<String, String>();
    let a = g.add_node("A".into());
    let b = g.add_node("B".into());
    let c = g.add_node("C".into());
    let d = g.add_node("D".into());
    g.add_edge(a, b, "ab".into());
    g.add_edge(a, c, "ac".into());
    g.add_edge(b, d, "bd".into());
    g.add_edge(c, d, "cd".into());
    (g, [a, b, c, d])
}

// go test: internal/graph/graph_test.go:25 buildChainGraph 4cb937007662cd25
// buildChainGraph creates: A→B→C
fn build_chain_graph() -> (Graph<String, String>, [NodeIndex; 3]) {
    let mut g = new::<String, String>();
    let a = g.add_node("A".into());
    let b = g.add_node("B".into());
    let c = g.add_node("C".into());
    g.add_edge(a, b, "ab".into());
    g.add_edge(b, c, "bc".into());
    (g, [a, b, c])
}

// go test: internal/graph/graph_test.go:36 collectValues 160671dec14be39d
// collectValues collects the values from an iter.Seq2 into a slice.
fn collect_values<'a, K, V: Clone + 'a>(seq: impl Iterator<Item = (K, &'a V)>) -> Vec<V> {
    let mut result = Vec::new();
    for (_, v) in seq {
        result.push(v.clone());
    }
    result
}

// go test: internal/graph/graph_test.go:45 collectIndices 3a6c1927b03bd274
// collectIndices collects the indices from an iter.Seq2 into a slice.
fn collect_indices<V>(seq: impl Iterator<Item = (i64, V)>) -> Vec<i64> {
    let mut result = Vec::new();
    for (idx, _) in seq {
        result.push(idx);
    }
    result
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(ToString::to_string).collect()
}

// go test: internal/graph/graph_test.go:53 TestNodeOperations c2cf403b4f64ed22
#[test]
fn test_node_operations() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("Add", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            let c = g.add_node("C".into());
            if a != 0 || b != 1 || c != 2 {
                t.error(sprintf!("expected sequential indices 0,1,2 but got %d,%d,%d", a, b, c));
            }
            for idx in [a, b, c] {
                if g.get_node(idx).is_none() {
                    t.error(sprintf!("node %d not found after Add", idx));
                }
            }
        });

        t.run("GetExisting", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("hello".into());
            let Some(data) = g.get_node(a) else {
                t.fatal("expected node to exist");
            };
            if data != "hello" {
                t.error(sprintf!("expected data %q, got %q", "hello", data));
            }
        });

        t.run("GetNonExisting", |t| {
            t.parallel();
            let g = new::<String, String>();
            let data = g.get_node(999);
            if data.is_some() {
                t.fatal("expected node not to exist");
            }
            // Go returns the zero value with ok == false.
            let data = data.cloned().unwrap_or_default();
            if !data.is_empty() {
                t.error(sprintf!("expected zero value, got %q", data));
            }
        });

        t.run("Has", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            if !g.has_node(a) {
                t.error("expected HasNode to return true for existing node");
            }
            if g.has_node(999) {
                t.error("expected HasNode to return false for non-existing node");
            }
        });

        t.run("Update", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("old".into());
            g.update_node(a, |_| "new".into());
            let data = g.get_node(a).cloned().unwrap_or_default();
            if data != "new" {
                t.error(sprintf!("expected %q after update, got %q", "new", data));
            }
        });

        t.run("UpdateNonExisting", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            // Should not panic
            g.update_node(999, |_| "x".into());
        });

        t.run("Remove", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            g.add_edge(a, b, "ab".into());
            g.remove_node(a);
            if g.has_node(a) {
                t.error("expected node to be removed");
            }
            if g.edge_count() != 0 {
                t.error(sprintf!("expected 0 edges after removing node, got %d", g.edge_count()));
            }
        });

        t.run("RemoveSelfLoop", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            g.add_edge(a, a, "self".into());
            if g.edge_count() != 1 {
                t.fatal(sprintf!("expected 1 edge, got %d", g.edge_count()));
            }
            g.remove_node(a);
            if g.edge_count() != 0 {
                t.error(sprintf!(
                    "expected 0 edges after removing self-loop node, got %d",
                    g.edge_count()
                ));
            }
        });

        t.run("Count", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            if g.node_count() != 0 {
                t.error(sprintf!("expected 0, got %d", g.node_count()));
            }
            let a = g.add_node("A".into());
            g.add_node("B".into());
            if g.node_count() != 2 {
                t.error(sprintf!("expected 2, got %d", g.node_count()));
            }
            g.remove_node(a);
            if g.node_count() != 1 {
                t.error(sprintf!("expected 1 after remove, got %d", g.node_count()));
            }
        });
    });
}

// go test: internal/graph/graph_test.go:173 TestEdgeOperations 09680cb9bcd5c5cc
#[test]
fn test_edge_operations() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("Add", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            let ei = g.add_edge(a, b, "ab".into());
            if !g.has_edge(a, b) {
                t.error("expected HasEdge to return true");
            }
            let Some(edge) = g.get_edge(ei) else {
                t.fatal("expected edge to exist");
            };
            if edge.data != "ab" {
                t.error(sprintf!("expected edge data %q, got %q", "ab", edge.data));
            }
        });

        t.run("PanicsOnInvalidSource", |t| {
            t.parallel();
            let r = catch_unwind(AssertUnwindSafe(|| {
                let mut g = new::<String, String>();
                g.add_node("A".into());
                g.add_edge(999, 0, "x".into());
            }));
            if r.is_ok() {
                t.error("expected panic for invalid source node");
            }
        });

        t.run("PanicsOnInvalidTarget", |t| {
            t.parallel();
            let r = catch_unwind(AssertUnwindSafe(|| {
                let mut g = new::<String, String>();
                g.add_node("A".into());
                g.add_edge(0, 999, "x".into());
            }));
            if r.is_ok() {
                t.error("expected panic for invalid target node");
            }
        });

        t.run("GetExisting", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            let ei = g.add_edge(a, b, "ab".into());
            let Some(edge) = g.get_edge(ei) else {
                t.fatal("expected edge to exist");
            };
            if edge.source != a || edge.target != b || edge.data != "ab" {
                // Go's %+v of the Edge struct.
                t.error(sprintf!(
                    "unexpected edge: {Source:%d Target:%d Data:%v}",
                    edge.source,
                    edge.target,
                    edge.data
                ));
            }
        });

        t.run("GetNonExisting", |t| {
            t.parallel();
            let g = new::<String, String>();
            if g.get_edge(999).is_some() {
                t.error("expected edge not to exist");
            }
        });

        t.run("Has", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            g.add_edge(a, b, "ab".into());
            if !g.has_edge(a, b) {
                t.error("expected HasEdge true for existing edge");
            }
            if g.has_edge(b, a) {
                t.error("expected HasEdge false for non-existing edge");
            }
        });

        t.run("Update", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            let ei = g.add_edge(a, b, "old".into());
            g.update_edge(ei, |_| "new".into());
            let data = g.get_edge(ei).map(|edge| edge.data.clone()).unwrap_or_default();
            if data != "new" {
                t.error(sprintf!("expected %q, got %q", "new", data));
            }
        });

        t.run("UpdateNonExisting", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            // Should not panic
            g.update_edge(999, |_| "x".into());
        });

        t.run("Remove", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            let ei = g.add_edge(a, b, "ab".into());
            g.remove_edge(ei);
            if g.has_edge(a, b) {
                t.error("expected edge to be removed");
            }
            if !g.has_node(a) || !g.has_node(b) {
                t.error("expected nodes to be preserved");
            }
        });

        t.run("Count", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            if g.edge_count() != 0 {
                t.error(sprintf!("expected 0, got %d", g.edge_count()));
            }
            let ei = g.add_edge(a, b, "ab".into());
            if g.edge_count() != 1 {
                t.error(sprintf!("expected 1, got %d", g.edge_count()));
            }
            g.remove_edge(ei);
            if g.edge_count() != 0 {
                t.error(sprintf!("expected 0 after remove, got %d", g.edge_count()));
            }
        });

        t.run("MultipleSamePair", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            g.add_edge(a, b, "first".into());
            g.add_edge(a, b, "second".into());
            if g.edge_count() != 2 {
                t.error(sprintf!("expected 2 edges, got %d", g.edge_count()));
            }
        });
    });
}

// go test: internal/graph/graph_test.go:321 TestIteration 27bc795b2d0a5122
#[test]
fn test_iteration() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("Nodes", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            let values = collect_values(g.nodes());
            let expected = strings(&["A", "B", "C", "D"]);
            if values != expected {
                t.error(sprintf!("expected %v, got %v", expected, values));
            }
        });

        t.run("Edges", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            let edges = collect_values(g.edges());
            if edges.len() != 4 {
                t.fatal(sprintf!("expected 4 edges, got %d", edges.len()));
            }
            // Edges should be in insertion order (sorted by index)
            if edges[0].source != n[0] || edges[0].target != n[1] || edges[0].data != "ab" {
                t.error(sprintf!(
                    "unexpected first edge: {Source:%d Target:%d Data:%v}",
                    edges[0].source,
                    edges[0].target,
                    edges[0].data
                ));
            }
            if edges[3].source != n[2] || edges[3].target != n[3] || edges[3].data != "cd" {
                t.error(sprintf!(
                    "unexpected last edge: {Source:%d Target:%d Data:%v}",
                    edges[3].source,
                    edges[3].target,
                    edges[3].data
                ));
            }
        });

        t.run("Deterministic", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            let first = collect_values(g.nodes());
            let second = collect_values(g.nodes());
            if first != second {
                t.error(sprintf!("iteration not deterministic: %v vs %v", first, second));
            }
        });
    });
}

// go test: internal/graph/graph_test.go:360 TestQuerying 13e58fa09f902d45
#[test]
fn test_querying() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("FindNodeMatching", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            let Some(idx) = g.find_node(|s| s == "C") else {
                t.fatal("expected to find node");
            };
            if idx != n[2] {
                t.error(sprintf!("expected index %d, got %d", n[2], idx));
            }
        });

        t.run("FindNodeNone", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            if g.find_node(|s| s == "Z").is_some() {
                t.error("expected not to find node");
            }
        });

        t.run("FindNodesMultiple", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            let indices = g.find_nodes(|s| s == "B" || s == "C");
            let expected = vec![n[1], n[2]];
            if indices != expected {
                t.error(sprintf!("expected %v, got %v", expected, indices));
            }
        });

        t.run("FindNodesNone", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            let indices = g.find_nodes(|s| s == "Z");
            if !indices.is_empty() {
                t.error(sprintf!("expected empty, got %v", indices));
            }
        });

        t.run("FindEdgeMatching", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            let Some(idx) = g.find_edge(|data, _, _| data == "bd") else {
                t.fatal("expected to find edge");
            };
            let data = g.get_edge(idx).map(|edge| edge.data.clone()).unwrap_or_default();
            if data != "bd" {
                t.error(sprintf!("expected data %q, got %q", "bd", data));
            }
        });

        t.run("FindEdgeNone", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            if g.find_edge(|data, _, _| data == "zz").is_some() {
                t.error("expected not to find edge");
            }
        });

        t.run("FindEdgesMultiple", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            let indices = g.find_edges(|data, _, _| data == "ab" || data == "cd");
            if indices.len() != 2 {
                t.error(sprintf!("expected 2 edges, got %d", indices.len()));
            }
        });

        t.run("FindEdgesNone", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            let indices = g.find_edges(|_, _, _| false);
            if !indices.is_empty() {
                t.error(sprintf!("expected empty, got %v", indices));
            }
        });
    });
}

// go test: internal/graph/graph_test.go:445 TestNeighborhood 761620b51d1fc7f1
#[test]
fn test_neighborhood() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("Neighbors", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            // B's neighbors: A (incoming) and D (outgoing)
            let neighbors = g.neighbors(n[1]);
            let expected = vec![n[0], n[3]];
            if neighbors != expected {
                t.error(sprintf!("expected %v, got %v", expected, neighbors));
            }
        });

        t.run("NeighborsDirectedOutgoing", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            // A's outgoing: B, C
            let neighbors = g.neighbors_directed(n[0], Outgoing);
            let expected = vec![n[1], n[2]];
            if neighbors != expected {
                t.error(sprintf!("expected %v, got %v", expected, neighbors));
            }
        });

        t.run("NeighborsDirectedIncoming", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            // D's incoming: B, C
            let neighbors = g.neighbors_directed(n[3], Incoming);
            let expected = vec![n[1], n[2]];
            if neighbors != expected {
                t.error(sprintf!("expected %v, got %v", expected, neighbors));
            }
        });

        t.run("OutgoingEdges", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            let edges = g.outgoing_edges(n[0]);
            let expected: Vec<EdgeIndex> = vec![0, 1];
            if edges != expected {
                t.error(sprintf!("expected %v, got %v", expected, edges));
            }
        });

        t.run("IncomingEdges", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            let edges = g.incoming_edges(n[3]);
            let expected: Vec<EdgeIndex> = vec![2, 3];
            if edges != expected {
                t.error(sprintf!("expected %v, got %v", expected, edges));
            }
        });
    });
}

// go test: internal/graph/graph_test.go:501 TestExternals abbdae212e5aa5b5
#[test]
fn test_externals() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("Sinks", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            let sinks = collect_values(g.externals(Outgoing));
            let expected = strings(&["D"]);
            if sinks != expected {
                t.error(sprintf!("expected %v, got %v", expected, sinks));
            }
            let indices = collect_indices(g.externals(Outgoing));
            if indices != [n[3]] {
                t.error(sprintf!("expected sink index %v, got %v", vec![n[3]], indices));
            }
        });

        t.run("Sources", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            let sources = collect_values(g.externals(Incoming));
            let expected = strings(&["A"]);
            if sources != expected {
                t.error(sprintf!("expected %v, got %v", expected, sources));
            }
            let indices = collect_indices(g.externals(Incoming));
            if indices != [n[0]] {
                t.error(sprintf!("expected source index %v, got %v", vec![n[0]], indices));
            }
        });
    });
}

// go test: internal/graph/graph_test.go:532 TestTransformations bc1e3ee154cb4efe
#[test]
fn test_transformations() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("Reverse", |t| {
            t.parallel();
            let (mut g, n) = build_diamond_graph();
            g.reverse();
            // After reverse, edge a→b becomes b→a
            if !g.has_edge(n[1], n[0]) {
                t.error("expected edge B→A after reverse");
            }
            if g.has_edge(n[0], n[1]) {
                t.error("expected no edge A→B after reverse");
            }
        });

        t.run("FilterNodes", |t| {
            t.parallel();
            let (mut g, _) = build_diamond_graph();
            // Keep only A and B
            g.filter_nodes(|s| s == "A" || s == "B");
            if g.node_count() != 2 {
                t.error(sprintf!("expected 2 nodes, got %d", g.node_count()));
            }
            // Only edge A→B should remain (edges to C and D removed)
            if g.edge_count() != 1 {
                t.error(sprintf!("expected 1 edge, got %d", g.edge_count()));
            }
        });

        t.run("FilterEdges", |t| {
            t.parallel();
            let (mut g, _) = build_diamond_graph();
            g.filter_edges(|s| s == "ab" || s == "cd");
            if g.edge_count() != 2 {
                t.error(sprintf!("expected 2 edges, got %d", g.edge_count()));
            }
            // All nodes should be preserved
            if g.node_count() != 4 {
                t.error(sprintf!("expected 4 nodes, got %d", g.node_count()));
            }
        });

        // Rust-only: Go's strings.ToUpper; the inputs are ASCII, where it is ASCII upper-casing.
        let to_upper = |s: &String| s.to_ascii_uppercase();

        t.run("MapNodes", |t| {
            t.parallel();
            let (mut g, n) = build_diamond_graph();
            g.map_nodes(to_upper);
            let data = g.get_node(n[0]).cloned().unwrap_or_default();
            if data != "A" {
                t.error(sprintf!("expected %q, got %q", "A", data));
            }
            // Already uppercase, let's use lowercase first
            let mut g2 = new::<String, String>();
            let idx = g2.add_node("hello".into());
            g2.map_nodes(to_upper);
            let d = g2.get_node(idx).cloned().unwrap_or_default();
            if d != "HELLO" {
                t.error(sprintf!("expected %q, got %q", "HELLO", d));
            }
        });

        t.run("MapEdges", |t| {
            t.parallel();
            let (mut g, _) = build_diamond_graph();
            g.map_edges(to_upper);
            let edges = collect_values(g.edges());
            if edges[0].data != "AB" {
                t.error(sprintf!("expected %q, got %q", "AB", edges[0].data));
            }
        });
    });
}

// go test: internal/graph/graph_test.go:603 TestClone 071e5abfed670992
#[test]
fn test_clone() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("MatchesOriginal", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            let c = g.clone();
            if c.node_count() != g.node_count() {
                t.error(sprintf!("node count mismatch: %d vs %d", c.node_count(), g.node_count()));
            }
            if c.edge_count() != g.edge_count() {
                t.error(sprintf!("edge count mismatch: %d vs %d", c.edge_count(), g.edge_count()));
            }
            let orig_nodes = collect_values(g.nodes());
            let clone_nodes = collect_values(c.nodes());
            if orig_nodes != clone_nodes {
                t.error(sprintf!("node data mismatch: %v vs %v", orig_nodes, clone_nodes));
            }
        });

        t.run("MutateClone", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            let mut c = g.clone();
            c.add_node("E".into());
            if g.node_count() != 4 {
                t.error(sprintf!("original should be unaffected, got %d nodes", g.node_count()));
            }
        });

        t.run("MutateOriginal", |t| {
            t.parallel();
            let (mut g, _) = build_diamond_graph();
            let c = g.clone();
            g.add_node("E".into());
            if c.node_count() != 4 {
                t.error(sprintf!("clone should be unaffected, got %d nodes", c.node_count()));
            }
        });
    });
}

// Rust-only: a traversal subtest's comparison (Go's `slices.Equal` and `%v` message).
fn expect_values(t: &ets_go::testing::T, result: &[String], expected: &[&str]) {
    let expected = strings(expected);
    if result != expected.as_slice() {
        t.error(sprintf!("expected %v, got %v", expected, result.to_vec()));
    }
}

fn start(nodes: &[NodeIndex]) -> TraversalConfig {
    TraversalConfig { start: nodes.to_vec(), direction: None }
}

// go test: internal/graph/graph_test.go:643 TestDFS cb992e3e7f67f989
#[test]
fn test_dfs() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("Chain", |t| {
            t.parallel();
            let (g, _) = build_chain_graph();
            let result = collect_values(g.dfs(TraversalConfig::default()));
            expect_values(t, &result, &["A", "B", "C"]);
        });

        t.run("Diamond", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            let result = collect_values(g.dfs(TraversalConfig::default()));
            // A first, then B (lower edge-index), then D (B's child), then C
            expect_values(t, &result, &["A", "B", "D", "C"]);
        });

        t.run("CustomStart", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            let result = collect_values(g.dfs(start(&[n[1]])));
            expect_values(t, &result, &["B", "D"]);
        });

        t.run("IncomingDirection", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            let result = collect_values(
                g.dfs(TraversalConfig { start: vec![n[3]], direction: Some(Incoming) }),
            );
            // From D following incoming edges: D, then B (lower edge-index bd<cd), then A, then C
            expect_values(t, &result, &["D", "B", "A", "C"]);
        });

        t.run("Disconnected", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            let c = g.add_node("C".into());
            let d = g.add_node("D".into());
            g.add_edge(a, b, "ab".into());
            g.add_edge(c, d, "cd".into());
            let result = collect_values(g.dfs(TraversalConfig::default()));
            expect_values(t, &result, &["A", "B", "C", "D"]);
        });

        t.run("EarlyTermination", |t| {
            t.parallel();
            let (g, _) = build_chain_graph();
            let mut result: Vec<String> = Vec::new();
            #[expect(clippy::never_loop, reason = "Go's `for ... { ...; break }`")]
            for (_, v) in g.dfs(TraversalConfig::default()) {
                result.push(v.clone());
                break;
            }
            if result.len() != 1 {
                t.error(sprintf!("expected 1 element, got %d", result.len()));
            }
            if result[0] != "A" {
                t.error(sprintf!("expected %q, got %q", "A", result[0]));
            }
        });
    });
}

// go test: internal/graph/graph_test.go:720 TestDFSPostOrder 48b9ced4f11bc288
#[test]
fn test_dfs_post_order() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("Chain", |t| {
            t.parallel();
            let (g, _) = build_chain_graph();
            let result = collect_values(g.dfs_post_order(TraversalConfig::default()));
            expect_values(t, &result, &["C", "B", "A"]);
        });

        t.run("Diamond", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            let result = collect_values(g.dfs_post_order(TraversalConfig::default()));
            // D first (deepest via B path), then B, then C (D already visited), then A
            expect_values(t, &result, &["D", "B", "C", "A"]);
        });

        t.run("CustomStart", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            let result = collect_values(g.dfs_post_order(start(&[n[1]])));
            expect_values(t, &result, &["D", "B"]);
        });
    });
}

// go test: internal/graph/graph_test.go:754 TestBFS f594f397b7cddce4
#[test]
fn test_bfs() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("Chain", |t| {
            t.parallel();
            let (g, _) = build_chain_graph();
            let result = collect_values(g.bfs(TraversalConfig::default()));
            expect_values(t, &result, &["A", "B", "C"]);
        });

        t.run("Diamond", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            let result = collect_values(g.bfs(TraversalConfig::default()));
            // A (level 0), B and C (level 1), D (level 2)
            expect_values(t, &result, &["A", "B", "C", "D"]);
        });

        t.run("CustomStart", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            let result = collect_values(g.bfs(start(&[n[1]])));
            expect_values(t, &result, &["B", "D"]);
        });

        t.run("IncomingDirection", |t| {
            t.parallel();
            let (g, n) = build_diamond_graph();
            let result = collect_values(
                g.bfs(TraversalConfig { start: vec![n[3]], direction: Some(Incoming) }),
            );
            // From D: D, then B and C (incoming neighbors), then A
            expect_values(t, &result, &["D", "B", "C", "A"]);
        });
    });
}

// go test: internal/graph/graph_test.go:799 TestTopo c677624343554bf5
#[test]
fn test_topo() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("Chain", |t| {
            t.parallel();
            let (g, _) = build_chain_graph();
            let result = collect_values(g.topo());
            expect_values(t, &result, &["A", "B", "C"]);
        });

        t.run("Diamond", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            let result = collect_values(g.topo());
            // A first, then B and C (in index order), then D
            expect_values(t, &result, &["A", "B", "C", "D"]);
        });

        t.run("MultipleRoots", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let r1 = g.add_node("R1".into());
            let r2 = g.add_node("R2".into());
            let tt = g.add_node("T".into());
            g.add_edge(r1, tt, "r1t".into());
            g.add_edge(r2, tt, "r2t".into());
            let result = collect_values(g.topo());
            expect_values(t, &result, &["R1", "R2", "T"]);
        });

        t.run("Cyclic", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            let c = g.add_node("C".into());
            g.add_edge(a, b, "ab".into());
            g.add_edge(b, c, "bc".into());
            g.add_edge(c, a, "ca".into());
            let result = collect_values(g.topo());
            if !result.is_empty() {
                t.error(sprintf!("expected empty result for cyclic graph, got %v", result));
            }
        });
    });
}

// go test: internal/graph/graph_test.go:853 TestIsAcyclic 4235fd9149dff073
#[test]
fn test_is_acyclic() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("DAG", |t| {
            t.parallel();
            let (g, _) = build_diamond_graph();
            if !g.is_acyclic() {
                t.error("expected diamond graph to be acyclic");
            }
        });

        t.run("WithCycle", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            let c = g.add_node("C".into());
            g.add_edge(a, b, "ab".into());
            g.add_edge(b, c, "bc".into());
            g.add_edge(c, a, "ca".into());
            if g.is_acyclic() {
                t.error("expected cyclic graph to not be acyclic");
            }
        });

        t.run("EmptyGraph", |t| {
            t.parallel();
            let g = new::<String, String>();
            if !g.is_acyclic() {
                t.error("expected empty graph to be acyclic");
            }
        });
    });
}

// Rust-only: Go's `MermaidOptions[string, string]{...}` shape and label functions.
fn shape(open: &'static str, close: &'static str) -> MermaidShape<'static, String> {
    Box::new(move |_: &String| (open.to_string(), close.to_string()))
}

// go test: internal/graph/graph_test.go:886 TestToMermaid 31d3fee59bf318c1
#[test]
fn test_to_mermaid() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("Simple", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            g.add_edge(a, b, "ab".into());
            let result = g.to_mermaid(MermaidOptions::default());
            let expected = "flowchart TB\n  0[\"A\"]\n  1[\"B\"]\n  0 -->|\"ab\"| 1";
            if result != expected {
                t.error(sprintf!("expected:\n%s\ngot:\n%s", expected, result));
            }
        });

        t.run("CustomLabels", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            g.add_edge(a, b, "ab".into());
            let result = g.to_mermaid(MermaidOptions {
                node_label: Box::new(|s: &String| format!("node:{s}")),
                edge_label: Box::new(|s: &String| format!("edge:{s}")),
                ..MermaidOptions::default()
            });
            if !result.contains("node:A") {
                t.error("expected custom node label prefix");
            }
            if !result.contains("edge:ab") {
                t.error("expected custom edge label prefix");
            }
        });

        t.run("CustomNodeShape", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            g.add_edge(a, b, "ab".into());
            let result = g.to_mermaid(MermaidOptions {
                node_shape: Some(shape("([", "])")),
                ..MermaidOptions::default()
            });
            if !result.contains("0([\"A\"])") {
                t.error(sprintf!("expected custom node shape, got %q", result));
            }
        });

        t.run("CustomEdgeShape", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            g.add_edge(a, b, "ab".into());
            let result = g.to_mermaid(MermaidOptions {
                edge_shape: Some(shape("-.", ".->")),
                ..MermaidOptions::default()
            });
            if !result.contains("0 -.|\"ab\"|.-> 1") {
                t.error(sprintf!("expected custom edge shape, got %q", result));
            }
        });

        t.run("Direction", |t| {
            t.parallel();
            let g = new::<String, String>();
            let result = g.to_mermaid(MermaidOptions {
                direction: "LR".to_string(),
                ..MermaidOptions::default()
            });
            if !result.starts_with("flowchart LR") {
                t.error(sprintf!("expected flowchart LR, got %q", result));
            }
        });

        t.run("Escaping", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            g.add_node(r#"#a\b"c<d>[e]{f}(g)|h&i"#.to_string() + "\n" + "j");
            let result = g.to_mermaid(MermaidOptions::default());
            for (entity, message) in [
                ("#35;", "expected escaped hash entity"),
                ("#quot;", "expected escaped quote entity"),
                ("#lt;", "expected escaped less-than entity"),
                ("#gt;", "expected escaped greater-than entity"),
                ("#amp;", "expected escaped ampersand entity"),
                ("#91;", "expected escaped open bracket entity"),
                ("#93;", "expected escaped close bracket entity"),
                ("#123;", "expected escaped open brace entity"),
                ("#125;", "expected escaped close brace entity"),
                ("#40;", "expected escaped open parenthesis entity"),
                ("#41;", "expected escaped close parenthesis entity"),
                ("#124;", "expected escaped pipe entity"),
                ("#92;", "expected escaped backslash entity"),
                ("<br/>", "expected escaped newline as <br/>"),
            ] {
                if !result.contains(entity) {
                    t.error(message);
                }
            }
        });

        t.run("EmptyGraph", |t| {
            t.parallel();
            let g = new::<String, String>();
            let result = g.to_mermaid(MermaidOptions::default());
            if result != "flowchart TB" {
                t.error(sprintf!("expected %q, got %q", "flowchart TB", result));
            }
        });

        t.run("EmptyEdgeLabel", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            g.add_edge(a, b, "ab".into());
            let result = g.to_mermaid(MermaidOptions {
                edge_label: Box::new(|_: &String| String::new()),
                ..MermaidOptions::default()
            });
            if !result.contains("-->") {
                t.error("expected --> in output");
            }
            if result.contains("-->|") {
                t.error("expected no label syntax for empty edge label");
            }
        });
    });
}

// go test: internal/graph/graph_test.go:1035 TestEdgeCases 3fd78ed79b0f6bad
#[test]
fn test_edge_cases() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("EmptyGraph", |t| {
            t.parallel();
            let g = new::<String, String>();
            if g.node_count() != 0 {
                t.error(sprintf!("expected 0 nodes, got %d", g.node_count()));
            }
            if g.edge_count() != 0 {
                t.error(sprintf!("expected 0 edges, got %d", g.edge_count()));
            }
            let nodes = collect_values(g.nodes());
            if !nodes.is_empty() {
                t.error(sprintf!("expected nil, got %v", nodes));
            }
            if !g.is_acyclic() {
                t.error("expected empty graph to be acyclic");
            }
        });

        t.run("SelfLoop", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            g.add_edge(a, a, "self".into());
            if !g.has_edge(a, a) {
                t.error("expected self-loop edge");
            }
            g.remove_node(a);
            if g.edge_count() != 0 {
                t.error(sprintf!(
                    "expected 0 edges after removing self-loop node, got %d",
                    g.edge_count()
                ));
            }
        });

        t.run("RemoveLastNode", |t| {
            t.parallel();
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            g.remove_node(a);
            if g.node_count() != 0 {
                t.error(sprintf!("expected 0 nodes, got %d", g.node_count()));
            }
        });

        t.run("TraversalEmptyGraph", |t| {
            t.parallel();
            let g = new::<String, String>();
            let dfs = collect_values(g.dfs(TraversalConfig::default()));
            if !dfs.is_empty() {
                t.error(sprintf!("expected nil DFS result, got %v", dfs));
            }
            let bfs = collect_values(g.bfs(TraversalConfig::default()));
            if !bfs.is_empty() {
                t.error(sprintf!("expected nil BFS result, got %v", bfs));
            }
            let post = collect_values(g.dfs_post_order(TraversalConfig::default()));
            if !post.is_empty() {
                t.error(sprintf!("expected nil DFSPostOrder result, got %v", post));
            }
            let topo = collect_values(g.topo());
            if !topo.is_empty() {
                t.error(sprintf!("expected nil Topo result, got %v", topo));
            }
        });
    });
}
