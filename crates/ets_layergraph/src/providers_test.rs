// Rust: Go builds node infos with nil nodes and nil type slots, and edges with the zero LayerGraphEdgeInfo. A
// `P<Node>` or `P<Type>` cannot be null, so the magic tests' `Fixture` provides a real node and type (only the
// slice lengths matter); the edges carry a pipe relationship (Go's zero relationship "" is not a constant, and the
// walk never reads it).

use ets_go::sprintf;
use ets_go::testing::T;
use ets_graph::NodeIndex;

use crate::magic_test::{Fixture, run};
use crate::providers::walk_leaves_matching;
use crate::*;

// go test: internal/layergraph/providers_test.go:12 makeNodeInfo 1ab0fda115e9d07d
// makeNodeInfo creates a minimal LayerGraphNodeInfo for testing walkLeavesMatching.
// When hasTag is true, Provides gets a non-empty slice so the predicate matches.
fn make_node_info(fx: &Fixture, has_tag: bool) -> LayerGraphNodeInfo {
    let node = fx.calls[0];
    let mut info = LayerGraphNodeInfo {
        node,
        display_node: node,
        layer_type: None,
        provides: Vec::new(),
        actual_provides: Vec::new(),
        requires: Vec::new(),
    };
    if has_tag {
        info.provides = vec![fx.dummy];
    }
    info
}

// go test: internal/layergraph/providers_test.go:21 tagPredicate 2005249faa6d78f1
// tagPredicate returns true for nodes where Provides is non-empty.
fn tag_predicate(node: &LayerGraphNodeInfo) -> bool {
    !node.provides.is_empty()
}

/// Rust-only: Go's `LayerGraphEdgeInfo{}` (see the file comment).
fn edge() -> LayerGraphEdgeInfo {
    LayerGraphEdgeInfo::new(EdgeRelationship::Pipe)
}

fn walk(
    g: &ets_graph::Graph<LayerGraphNodeInfo, LayerGraphEdgeInfo>,
    start: &[NodeIndex],
) -> Vec<LayerGraphNodeInfo> {
    walk_leaves_matching(g, start, &mut tag_predicate)
}

fn expect_len(t: &T, result: &[LayerGraphNodeInfo], expected: usize, message: &str) {
    if result.len() != expected {
        t.fatal(sprintf!(message, result.len()));
    }
}

// go test: internal/layergraph/providers_test.go:25 TestWalkLeavesMatching 0d6c09fff38d8b67
#[test]
fn test_walk_leaves_matching() {
    run(|t| {
        t.parallel();
        let fx = Fixture::new();

        t.run("single node that matches", |t| {
            t.parallel();
            let mut g = ets_graph::new::<LayerGraphNodeInfo, LayerGraphEdgeInfo>();
            let n0 = g.add_node(make_node_info(&fx, true));

            let result = walk(&g, &[n0]);
            expect_len(t, &result, 1, "expected 1 result, got %d");
        });

        t.run("single node that does not match", |t| {
            t.parallel();
            let mut g = ets_graph::new::<LayerGraphNodeInfo, LayerGraphEdgeInfo>();
            let n0 = g.add_node(make_node_info(&fx, false));

            let result = walk(&g, &[n0]);
            expect_len(t, &result, 0, "expected 0 results, got %d");
        });

        t.run("linear chain, all match, emits deepest", |t| {
            t.parallel();
            // root(match) -> middle(match) -> leaf(match)
            // Should emit only the leaf (deepest match).
            let mut g = ets_graph::new::<LayerGraphNodeInfo, LayerGraphEdgeInfo>();
            let root = g.add_node(make_node_info(&fx, true));
            let middle = g.add_node(make_node_info(&fx, true));
            let leaf = g.add_node(make_node_info(&fx, true));
            g.add_edge(root, middle, edge());
            g.add_edge(middle, leaf, edge());

            let result = walk(&g, &[root]);
            expect_len(t, &result, 1, "expected 1 result (deepest leaf), got %d");
        });

        t.run("linear chain, only root matches", |t| {
            t.parallel();
            // root(match) -> middle(no) -> leaf(no)
            // root's outgoing neighbor (middle) doesn't match, so root is emitted.
            let mut g = ets_graph::new::<LayerGraphNodeInfo, LayerGraphEdgeInfo>();
            let root = g.add_node(make_node_info(&fx, true));
            let middle = g.add_node(make_node_info(&fx, false));
            let leaf = g.add_node(make_node_info(&fx, false));
            g.add_edge(root, middle, edge());
            g.add_edge(middle, leaf, edge());

            let result = walk(&g, &[root]);
            expect_len(t, &result, 1, "expected 1 result, got %d");
        });

        t.run("branching graph, multiple leaves match", |t| {
            t.parallel();
            // root(match) -> left(match), root -> right(match)
            // left and right are both leaf matches (no outgoing matching neighbors).
            let mut g = ets_graph::new::<LayerGraphNodeInfo, LayerGraphEdgeInfo>();
            let root = g.add_node(make_node_info(&fx, true));
            let left = g.add_node(make_node_info(&fx, true));
            let right = g.add_node(make_node_info(&fx, true));
            g.add_edge(root, left, edge());
            g.add_edge(root, right, edge());

            let result = walk(&g, &[root]);
            expect_len(t, &result, 2, "expected 2 results, got %d");
        });

        t.run("no nodes match predicate", |t| {
            t.parallel();
            let mut g = ets_graph::new::<LayerGraphNodeInfo, LayerGraphEdgeInfo>();
            let n0 = g.add_node(make_node_info(&fx, false));
            let n1 = g.add_node(make_node_info(&fx, false));
            g.add_edge(n0, n1, edge());

            let result = walk(&g, &[n0]);
            expect_len(t, &result, 0, "expected 0 results, got %d");
        });

        t.run("diamond graph, all match, emits only sink", |t| {
            t.parallel();
            // A(match) -> B(match), A -> C(match), B -> D(match), C -> D(match)
            // D is the deepest leaf match. Should only emit D.
            let mut g = ets_graph::new::<LayerGraphNodeInfo, LayerGraphEdgeInfo>();
            let a = g.add_node(make_node_info(&fx, true));
            let b = g.add_node(make_node_info(&fx, true));
            let c = g.add_node(make_node_info(&fx, true));
            let d = g.add_node(make_node_info(&fx, true));
            g.add_edge(a, b, edge());
            g.add_edge(a, c, edge());
            g.add_edge(b, d, edge());
            g.add_edge(c, d, edge());

            let result = walk(&g, &[a]);
            expect_len(t, &result, 1, "expected 1 result (D), got %d");
        });
    });
}
