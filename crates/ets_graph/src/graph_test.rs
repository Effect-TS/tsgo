use std::panic::{AssertUnwindSafe, catch_unwind};

use ets_go::sprintf;

use crate::*;

// go test: internal/graph/graph_test.go:10 buildDiamondGraph 0531fbdaa3aa84ef
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

// go test: internal/graph/graph_test.go:23 buildChainGraph cb9138c330aa713a
fn build_chain_graph() -> (Graph<String, String>, [NodeIndex; 3]) {
    let mut g = new::<String, String>();
    let a = g.add_node("A".into());
    let b = g.add_node("B".into());
    let c = g.add_node("C".into());
    g.add_edge(a, b, "ab".into());
    g.add_edge(b, c, "bc".into());
    (g, [a, b, c])
}

// go test: internal/graph/graph_test.go:33 buildCycleGraph 0e539f1ae5f87f85
fn build_cycle_graph() -> Graph<String, String> {
    let (mut g, n) = build_chain_graph();
    g.add_edge(n[2], n[0], "ca".into());
    g
}

// go test: internal/graph/graph_test.go:39 collectValues 5a331b4bdeb0719a
fn collect_values<'a, K, V: Clone + 'a>(seq: impl Iterator<Item = (K, &'a V)>) -> Vec<V> {
    let mut result = Vec::new();
    for (_, v) in seq {
        result.push(v.clone());
    }
    result
}

// go test: internal/graph/graph_test.go:47 collectIndices cec657207effe748
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

// go test: internal/graph/graph_test.go:55 TestNodeOperations d32e912822ad8bde
#[test]
fn test_node_operations() {
    ets_go::testing::run(|t| {
        t.parallel();
        let mut g = new::<String, String>();
        let a = g.add_node("A".into());
        let b = g.add_node("B".into());
        if a != 0 || b != 1 {
            t.fatal(sprintf!("expected sequential indices 0,1 but got %d,%d", a, b));
        }
        let data = g.get_node(a).cloned();
        if data.as_deref() != Some("A") || !g.has_node(a) {
            t.fatal(sprintf!(
                "GetNode(a) = %q, %v",
                data.clone().unwrap_or_default(),
                data.is_some()
            ));
        }
        let data = g.get_node(999).cloned();
        if data.is_some() || !data.clone().unwrap_or_default().is_empty() || g.has_node(999) {
            t.fatal(sprintf!(
                "GetNode(999) = %q, %v; want zero value, false",
                data.clone().unwrap_or_default(),
                data.is_some()
            ));
        }
        g.update_node(a, |_| "new".into());
        g.update_node(999, |_| "x".into());
        let data = g.get_node(a).cloned().unwrap_or_default();
        if data != "new" || g.node_count() != 2 {
            t.fatal(sprintf!("after update: data %q, count %d", data, g.node_count()));
        }
        g.add_edge(a, b, "ab".into());
        g.add_edge(a, a, "self".into());
        g.remove_node(a);
        if g.has_node(a) || g.node_count() != 1 || g.edge_count() != 0 {
            t.fatal(sprintf!(
                "after remove: has %v, nodes %d, edges %d",
                g.has_node(a),
                g.node_count(),
                g.edge_count()
            ));
        }
        g.remove_node(b);
        if g.node_count() != 0 {
            t.fatal(sprintf!("expected 0 nodes, got %d", g.node_count()));
        }
    });
}

// go test: internal/graph/graph_test.go:86 TestEdgeOperations 62c7225f5d176bc1
#[test]
fn test_edge_operations() {
    ets_go::testing::run(|t| {
        t.parallel();
        let mut g = new::<String, String>();
        let a = g.add_node("A".into());
        let b = g.add_node("B".into());
        let ei = g.add_edge(a, b, "old".into());
        let edge = g.get_edge(ei).cloned();
        if !edge
            .as_ref()
            .is_some_and(|edge| edge.source == a && edge.target == b && edge.data == "old")
        {
            // Rust: Go's `%+v` of the Edge struct.
            t.fatal(sprintf!("GetEdge = %v, %v", format!("{edge:?}"), edge.is_some()));
        }
        if !g.has_edge(a, b) || g.has_edge(b, a) {
            t.fatal("HasEdge must follow edge direction");
        }
        if g.get_edge(999).is_some() {
            t.fatal("expected edge 999 not to exist");
        }
        g.update_edge(ei, |_| "new".into());
        g.update_edge(999, |_| "x".into());
        let data = g.get_edge(ei).map(|edge| edge.data.clone()).unwrap_or_default();
        if data != "new" {
            t.fatal(sprintf!("expected updated edge data, got %q", data));
        }
        g.add_edge(a, b, "second".into());
        if g.edge_count() != 2 {
            t.fatal(sprintf!("expected parallel edges to be kept, got %d", g.edge_count()));
        }
        g.remove_edge(ei);
        if g.edge_count() != 1 || !g.has_node(a) || !g.has_node(b) {
            t.fatal(sprintf!(
                "RemoveEdge: edges %d, nodes kept %v",
                g.edge_count(),
                g.has_node(a) && g.has_node(b)
            ));
        }
        // Rust: Go ranges over a map, in random order; the entries are independent.
        let adds: [(&str, Box<dyn Fn(&mut Graph<String, String>)>); 2] = [
            ("invalid source", Box::new(move |g| _ = g.add_edge(999, a, "x".into()))),
            ("invalid target", Box::new(move |g| _ = g.add_edge(a, 999, "x".into()))),
        ];
        for (name, add) in adds {
            let r = catch_unwind(AssertUnwindSafe(|| add(&mut g)));
            if r.is_ok() {
                t.error(sprintf!("expected panic for %s", name));
            }
        }
    });
}

// go test: internal/graph/graph_test.go:129 TestIterationAndQuerying d56380d931e06cfb
#[test]
fn test_iteration_and_querying() {
    ets_go::testing::run(|t| {
        t.parallel();
        let (g, n) = build_diamond_graph();
        let values = collect_values(g.nodes());
        if values != strings(&["A", "B", "C", "D"]) {
            t.error(sprintf!("Nodes() = %v", values));
        }
        let mut edge_data = Vec::new();
        for (_, edge) in g.edges() {
            edge_data.push(edge.data.clone());
        }
        if edge_data != strings(&["ab", "ac", "bd", "cd"]) {
            t.error(sprintf!("Edges() = %v", edge_data));
        }
        let found = g.find_node(|s| s == "C");
        if found != Some(n[2]) {
            t.error(sprintf!("FindNode(C) = %d, %v", found.unwrap_or_default(), found.is_some()));
        }
        if g.find_node(|s| s == "Z").is_some() {
            t.error("FindNode(Z) found a node");
        }
        let got = g.find_nodes(|s| s == "B" || s == "C");
        if got != [n[1], n[2]] {
            t.error(sprintf!("FindNodes(B|C) = %v", got));
        }
        let got = g.find_nodes(|_| false);
        if !got.is_empty() {
            t.error(sprintf!("FindNodes(none) = %v", got));
        }
        let found = g.find_edge(|data, _, _| data == "bd");
        if found != Some(2) {
            t.error(sprintf!("FindEdge(bd) = %d, %v", found.unwrap_or_default(), found.is_some()));
        }
        if g.find_edge(|_, _, _| false).is_some() {
            t.error("FindEdge(none) found an edge");
        }
        let got = g.find_edges(|data, _, _| data == "ab" || data == "cd");
        if got != [0, 3] {
            t.error(sprintf!("FindEdges(ab|cd) = %v", got));
        }
        let got = g.find_edges(|_, _, _| false);
        if !got.is_empty() {
            t.error(sprintf!("FindEdges(none) = %v", got));
        }
    });
}

// go test: internal/graph/graph_test.go:168 TestNeighborhood 1dc58885783c774e
#[test]
fn test_neighborhood() {
    ets_go::testing::run(|t| {
        t.parallel();
        let (g, n) = build_diamond_graph();
        let tests: [(&str, Vec<i64>, Vec<i64>); 7] = [
            ("Neighbors", g.neighbors(n[1]), vec![n[0], n[3]]),
            ("NeighborsDirectedOutgoing", g.neighbors_directed(n[0], Outgoing), vec![n[1], n[2]]),
            ("NeighborsDirectedIncoming", g.neighbors_directed(n[3], Incoming), vec![n[1], n[2]]),
            ("OutgoingEdges", g.outgoing_edges(n[0]), vec![0, 1]),
            ("IncomingEdges", g.incoming_edges(n[3]), vec![2, 3]),
            ("Sinks", collect_indices(g.externals(Outgoing)), vec![n[3]]),
            ("Sources", collect_indices(g.externals(Incoming)), vec![n[0]]),
        ];
        for (name, got, expected) in tests {
            if got != expected {
                t.error(sprintf!("%s: expected %v, got %v", name, expected, got));
            }
        }
        let sinks = collect_values(g.externals(Outgoing));
        if sinks != strings(&["D"]) {
            t.error(sprintf!("sink values = %v", sinks));
        }
    });
}

// go test: internal/graph/graph_test.go:193 TestTransformations a98621bb28f4aa8e
#[test]
fn test_transformations() {
    ets_go::testing::run(|t| {
        t.parallel();
        t.run("Reverse", |t| {
            t.parallel();
            let (mut g, n) = build_diamond_graph();
            g.reverse();
            if !g.has_edge(n[1], n[0]) || g.has_edge(n[0], n[1]) {
                t.error("expected edge A→B to become B→A");
            }
        });

        t.run("FilterNodes", |t| {
            t.parallel();
            let (mut g, _) = build_diamond_graph();
            g.filter_nodes(|s| s == "A" || s == "B");
            if g.node_count() != 2 || g.edge_count() != 1 {
                t.error(sprintf!(
                    "expected 2 nodes and 1 edge, got %d and %d",
                    g.node_count(),
                    g.edge_count()
                ));
            }
        });

        t.run("FilterEdges", |t| {
            t.parallel();
            let (mut g, _) = build_diamond_graph();
            g.filter_edges(|s| s == "ab" || s == "cd");
            if g.node_count() != 4 || g.edge_count() != 2 {
                t.error(sprintf!(
                    "expected 4 nodes and 2 edges, got %d and %d",
                    g.node_count(),
                    g.edge_count()
                ));
            }
        });

        t.run("MapNodesAndEdges", |t| {
            t.parallel();
            let (mut g, _) = build_chain_graph();
            // Rust: Go's strings.ToLower / strings.ToUpper; the inputs are ASCII.
            g.map_nodes(|s| s.to_ascii_lowercase());
            g.map_edges(|s| s.to_ascii_uppercase());
            let nodes = collect_values(g.nodes());
            if nodes != strings(&["a", "b", "c"]) {
                t.error(sprintf!("mapped nodes = %v", nodes));
            }
            let edges = collect_values(g.edges());
            if edges[0].data != "AB" || edges[1].data != "BC" {
                t.error(sprintf!("mapped edges = %v", format!("{edges:?}")));
            }
        });

        t.run("CloneIsIndependent", |t| {
            t.parallel();
            let (mut g, _) = build_diamond_graph();
            let mut c = g.clone();
            if collect_values(c.nodes()) != collect_values(g.nodes())
                || collect_values(c.edges()) != collect_values(g.edges())
            {
                t.fatal("clone does not match original");
            }
            c.add_node("E".into());
            g.remove_edge(0);
            if g.node_count() != 4 || c.edge_count() != 4 {
                t.error(sprintf!(
                    "clone shares state: original nodes %d, clone edges %d",
                    g.node_count(),
                    c.edge_count()
                ));
            }
        });
    });
}

// go test: internal/graph/graph_test.go:250 TestTraversalOrder 72b2fa0fd24ff6c5
#[test]
fn test_traversal_order() {
    ets_go::testing::run(|t| {
        t.parallel();
        let (diamond, d) = build_diamond_graph();
        let (chain, _) = build_chain_graph();
        let mut disconnected = new::<String, String>();
        for name in ["A", "B", "C", "D"] {
            disconnected.add_node(name.into());
        }
        disconnected.add_edge(0, 1, "ab".into());
        disconnected.add_edge(2, 3, "cd".into());
        let mut multiple_roots = new::<String, String>();
        let r1 = multiple_roots.add_node("R1".into());
        let r2 = multiple_roots.add_node("R2".into());
        let target = multiple_roots.add_node("T".into());
        multiple_roots.add_edge(r1, target, "r1t".into());
        multiple_roots.add_edge(r2, target, "r2t".into());
        let empty = new::<String, String>();
        let from_b = || TraversalConfig { start: vec![d[1]], direction: None };
        let from_d_incoming = || TraversalConfig { start: vec![d[3]], direction: Some(Incoming) };
        let cycle = build_cycle_graph();

        // Rust: the sequences are collected when the table is built (Go collects them in the loop).
        let tests: [(&str, Vec<String>, &[&str]); 20] = [
            ("DFS/Chain", collect_values(chain.dfs(TraversalConfig::default())), &["A", "B", "C"]),
            (
                "DFS/Diamond",
                collect_values(diamond.dfs(TraversalConfig::default())),
                &["A", "B", "D", "C"],
            ),
            ("DFS/CustomStart", collect_values(diamond.dfs(from_b())), &["B", "D"]),
            (
                "DFS/IncomingDirection",
                collect_values(diamond.dfs(from_d_incoming())),
                &["D", "B", "A", "C"],
            ),
            (
                "DFS/Disconnected",
                collect_values(disconnected.dfs(TraversalConfig::default())),
                &["A", "B", "C", "D"],
            ),
            ("DFS/Empty", collect_values(empty.dfs(TraversalConfig::default())), &[]),
            (
                "DFSPostOrder/Chain",
                collect_values(chain.dfs_post_order(TraversalConfig::default())),
                &["C", "B", "A"],
            ),
            (
                "DFSPostOrder/Diamond",
                collect_values(diamond.dfs_post_order(TraversalConfig::default())),
                &["D", "B", "C", "A"],
            ),
            (
                "DFSPostOrder/CustomStart",
                collect_values(diamond.dfs_post_order(from_b())),
                &["D", "B"],
            ),
            (
                "DFSPostOrder/Empty",
                collect_values(empty.dfs_post_order(TraversalConfig::default())),
                &[],
            ),
            ("BFS/Chain", collect_values(chain.bfs(TraversalConfig::default())), &["A", "B", "C"]),
            (
                "BFS/Diamond",
                collect_values(diamond.bfs(TraversalConfig::default())),
                &["A", "B", "C", "D"],
            ),
            ("BFS/CustomStart", collect_values(diamond.bfs(from_b())), &["B", "D"]),
            (
                "BFS/IncomingDirection",
                collect_values(diamond.bfs(from_d_incoming())),
                &["D", "B", "C", "A"],
            ),
            ("BFS/Empty", collect_values(empty.bfs(TraversalConfig::default())), &[]),
            ("Topo/Chain", collect_values(chain.topo()), &["A", "B", "C"]),
            ("Topo/Diamond", collect_values(diamond.topo()), &["A", "B", "C", "D"]),
            ("Topo/MultipleRoots", collect_values(multiple_roots.topo()), &["R1", "R2", "T"]),
            ("Topo/Cyclic", collect_values(cycle.topo()), &[]),
            ("Topo/Empty", collect_values(empty.topo()), &[]),
        ];
        for (name, got, expected) in tests {
            let expected = strings(expected);
            if got != expected {
                t.error(sprintf!("%s: expected %v, got %v", name, expected, got));
            }
        }

        let mut visited: Vec<String> = Vec::new();
        #[expect(clippy::never_loop, reason = "Go's `for ... { ...; break }`")]
        for (_, v) in chain.dfs(TraversalConfig::default()) {
            visited.push(v.clone());
            break;
        }
        if visited != strings(&["A"]) {
            t.error(sprintf!("early termination visited %v", visited));
        }
    });
}

// go test: internal/graph/graph_test.go:311 TestIsAcyclic 2a1895769b900f49
#[test]
fn test_is_acyclic() {
    ets_go::testing::run(|t| {
        t.parallel();
        let (diamond, _) = build_diamond_graph();
        if !diamond.is_acyclic() || !new::<String, String>().is_acyclic() {
            t.error("expected diamond and empty graphs to be acyclic");
        }
        if build_cycle_graph().is_acyclic() {
            t.error("expected cyclic graph to not be acyclic");
        }
    });
}

// Rust-only: Go's `MermaidOptions[string, string]{...}` shape functions.
fn shape(open: &'static str, close: &'static str) -> MermaidShape<'static, String> {
    Box::new(move |_: &String| (open.to_string(), close.to_string()))
}

// go test: internal/graph/graph_test.go:322 TestToMermaid caa0c3e401be915f
#[test]
fn test_to_mermaid() {
    ets_go::testing::run(|t| {
        t.parallel();
        let chain = || {
            let mut g = new::<String, String>();
            let a = g.add_node("A".into());
            let b = g.add_node("B".into());
            g.add_edge(a, b, "ab".into());
            g
        };
        let mut escaped = new::<String, String>();
        escaped.add_node(r#"#a\b"c<d>[e]{f}(g)|h&i"#.to_string() + "\n" + "j");

        let tests: [(&str, Graph<String, String>, MermaidOptions<'static, String, String>, &str);
            6] = [
            (
                "Default",
                chain(),
                MermaidOptions::default(),
                "flowchart TB\n  0[\"A\"]\n  1[\"B\"]\n  0 -->|\"ab\"| 1",
            ),
            (
                "Empty",
                new::<String, String>(),
                MermaidOptions { direction: "LR".to_string(), ..MermaidOptions::default() },
                "flowchart LR",
            ),
            (
                "CustomLabels",
                chain(),
                MermaidOptions {
                    node_label: Box::new(|s: &String| format!("node:{s}")),
                    edge_label: Box::new(|s: &String| format!("edge:{s}")),
                    ..MermaidOptions::default()
                },
                "flowchart TB\n  0[\"node:A\"]\n  1[\"node:B\"]\n  0 -->|\"edge:ab\"| 1",
            ),
            (
                "CustomShapes",
                chain(),
                MermaidOptions {
                    node_shape: Some(shape("([", "])")),
                    edge_shape: Some(shape("-.", ".->")),
                    ..MermaidOptions::default()
                },
                "flowchart TB\n  0([\"A\"])\n  1([\"B\"])\n  0 -.|\"ab\"|.-> 1",
            ),
            (
                "EmptyEdgeLabel",
                chain(),
                MermaidOptions {
                    edge_label: Box::new(|_: &String| String::new()),
                    ..MermaidOptions::default()
                },
                "flowchart TB\n  0[\"A\"]\n  1[\"B\"]\n  0 --> 1",
            ),
            (
                "Escaping",
                escaped,
                MermaidOptions::default(),
                "flowchart TB\n  0[\"#35;a#92;b#quot;c#lt;d#gt;#91;e#93;#123;f#125;#40;g#41;#124;h#amp;i<br/>j\"]",
            ),
        ];
        for (name, graph, options, expected) in tests {
            let got = graph.to_mermaid(options);
            if got != expected {
                t.error(sprintf!("%s: expected:\n%s\ngot:\n%s", name, expected, got));
            }
        }
    });
}
