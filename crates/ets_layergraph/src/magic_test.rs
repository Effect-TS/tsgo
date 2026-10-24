// Rust: Go's tests build outline nodes from synthetic `&ast.Node{Kind: ast.KindCallExpression}` nodes and nil
// `*checker.Type` slots, and pass a nil type parser to ConvertOutlineGraphToLayerMagic. A `P<Node>` or `P<Type>`
// cannot be null, so `Fixture` compiles a small program whose call expressions stand for the synthetic nodes and
// whose one type fills the slots (only the slice lengths matter), and provides a real parser.

use std::sync::Arc;

use ets_go::testing::{self, T};
use ets_graph::NodeIndex;
use ets_shim::ast::{Kind, Node};
use ets_shim::checker::Type;
use ets_shim::compiler::{CheckerHandle, Program, ProgramOptions};
use ets_shim::core::context::Context;
use ets_shim::core::tspath::ComparePathsOptions;
use ets_shim::core::{CompilerOptions, P, ScriptTarget, Tristate};
use ets_shim::vfs::{FS, bundled, vfstest};
use ets_typeparser::TypeParser;

use crate::*;

// go test: internal/layergraph/magic_test.go:217 dummyType
// dummyType is a type alias used only for creating slices of the right length in tests.
// Since we only need len(Provides) and len(Requires) for ordering, actual type values don't matter.
pub(crate) type DummyType = Type;

pub(crate) fn run(f: impl FnOnce(&T) + Send + 'static) {
    ets_go::testing::on_checker_stack(move || testing::run(f));
}

/// Rust-only: the program standing in for Go's synthetic nodes and nil types (see the file comment).
pub(crate) struct Fixture {
    program: &'static Program,
    checker: CheckerHandle,
    pub(crate) calls: Vec<P<Node>>,
    next_call: usize,
    pub(crate) dummy: P<DummyType>,
}

impl Fixture {
    pub(crate) fn new() -> Fixture {
        let source = "declare const f: () => void;\nf(); f(); f(); f(); f(); f(); f(); f();\n";
        let fs: Arc<dyn FS> =
            Arc::new(bundled::wrap_fs(vfstest::from_map([("/.src/test.ts", source)], true)));
        let compiler_options = P::new(CompilerOptions {
            target: ScriptTarget::ESNext,
            strict: Tristate::True,
            ..Default::default()
        });
        let host =
            ets_shim::compiler::new_compiler_host("/", fs, &bundled::lib_path(), None, None, None);
        let config = ets_shim::tsoptions::new_parsed_command_line(
            compiler_options,
            vec!["/.src/test.ts".to_string()],
            Vec::new(),
            ComparePathsOptions {
                use_case_sensitive_file_names: true,
                current_directory: "/".into(),
            },
        );
        let mut options = ProgramOptions::new(P::new(config), host);
        options.single_threaded = Tristate::True;
        let program = ets_shim::compiler::new_program(options);
        let mut checker = program.get_type_checker(&Context::background());
        let sf = program.get_source_file("/.src/test.ts").expect("test source file");
        let mut calls = Vec::new();
        for &statement in sf.as_node().statements() {
            if statement.kind() == Kind::ExpressionStatement
                && let Some(expression) = statement.expression()
                && expression.kind() == Kind::CallExpression
            {
                calls.push(expression);
            }
        }
        let callee = calls[0].expression().expect("callee");
        let dummy = checker.get_type_at_location(callee);
        Fixture { program, checker, calls, next_call: 0, dummy }
    }

    fn type_parser(&mut self) -> TypeParser<'_> {
        ets_typeparser::new_type_parser(self.program, &mut self.checker)
    }

    // go test: internal/layergraph/magic_test.go:221 makeOutlineNode
    // makeOutlineNode creates a LayerOutlineGraphNodeInfo for testing with the given
    // number of provides and requires (using nil pointers, sufficient for ordering tests).
    fn make_outline_node(
        &mut self,
        num_provides: usize,
        num_requires: usize,
    ) -> LayerOutlineGraphNodeInfo {
        let node = self.calls[self.next_call];
        self.next_call += 1;
        LayerOutlineGraphNodeInfo {
            node,
            display_node: node,
            provides: vec![self.dummy; num_provides],
            actual_provides: vec![self.dummy; num_provides],
            requires: vec![self.dummy; num_requires],
        }
    }
}

// go test: internal/layergraph/magic_test.go:11 TestCompareNodesByOrder
#[test]
fn test_compare_nodes_by_order() {
    run(|t| {
        t.parallel();
        let mut fx = Fixture::new();
        let mut g = ets_graph::new::<LayerOutlineGraphNodeInfo, ()>();

        // Node A: 3 provides, 1 requires
        let idx_a = g.add_node(fx.make_outline_node(3, 1));
        // Node B: 1 provides, 2 requires
        let idx_b = g.add_node(fx.make_outline_node(1, 2));
        // Node C: 3 provides, 3 requires
        let idx_c = g.add_node(fx.make_outline_node(3, 3));

        // A (3 provides) should come before B (1 provides) — compare returns negative
        let cmp = compare_nodes_by_order(&g, idx_a, idx_b);
        if cmp >= 0 {
            t.error(ets_go::sprintf!("expected A < B (more provides first), got %d", cmp));
        }

        // B (1 provides) should come after A (3 provides) — compare returns positive
        let cmp = compare_nodes_by_order(&g, idx_b, idx_a);
        if cmp <= 0 {
            t.error(ets_go::sprintf!("expected B > A, got %d", cmp));
        }

        // A (3 provides, 1 requires) vs C (3 provides, 3 requires) — tiebreak by requires
        let cmp = compare_nodes_by_order(&g, idx_a, idx_c);
        if cmp <= 0 {
            t.error(ets_go::sprintf!(
                "expected A > C (C has more requires, so C first), got %d",
                cmp
            ));
        }
    });
}

// go test: internal/layergraph/magic_test.go:47 TestCompareNodesByOrderLayerShapes
#[test]
fn test_compare_nodes_by_order_layer_shapes() {
    run(|t| {
        t.parallel();
        let mut fx = Fixture::new();
        let mut g = ets_graph::new::<LayerOutlineGraphNodeInfo, ()>();

        let both_provides_and_requires = g.add_node(fx.make_outline_node(1, 1));
        let only_provides = g.add_node(fx.make_outline_node(1, 0));
        let no_provides_or_requires = g.add_node(fx.make_outline_node(0, 0));
        let only_requires = g.add_node(fx.make_outline_node(0, 1));

        let ordered: [NodeIndex; 4] =
            [both_provides_and_requires, only_provides, no_provides_or_requires, only_requires];
        for i in 0..ordered.len() - 1 {
            let cmp = compare_nodes_by_order(&g, ordered[i], ordered[i + 1]);
            if cmp >= 0 {
                t.fatal(ets_go::sprintf!(
                    "expected node at order %d to sort before order %d, got %d",
                    i as i64,
                    (i + 1) as i64,
                    cmp
                ));
            }
        }
    });
}

// go test: internal/layergraph/magic_test.go:69 TestDFSPostOrderWithOrder
#[test]
fn test_dfs_post_order_with_order() {
    run(|t| {
        t.parallel();
        let mut fx = Fixture::new();
        // Build a small graph:
        //   A (3 provides) -> B (1 provides)
        //   A -> C (2 provides)
        //   B -> D (0 provides)
        let mut g = ets_graph::new::<LayerOutlineGraphNodeInfo, ()>();

        let node_a = fx.make_outline_node(3, 0);
        let node_b = fx.make_outline_node(1, 0);
        let node_c = fx.make_outline_node(2, 0);
        let node_d = fx.make_outline_node(0, 0);

        let idx_a = g.add_node(node_a);
        let idx_b = g.add_node(node_b);
        let idx_c = g.add_node(node_c);
        let idx_d = g.add_node(node_d);

        g.add_edge(idx_a, idx_b, ());
        g.add_edge(idx_a, idx_c, ());
        g.add_edge(idx_b, idx_d, ());

        let result = dfs_post_order_with_order(&g, &[idx_a]);

        // Post-order: children before parents
        // A's neighbors sorted by provides desc: C (2) before B (1)
        // Visiting C first (post-order): C has no children -> emit C
        // Then B: B has child D -> emit D, then B
        // Finally A
        // Expected order: C, D, B, A
        if result.len() != 4 {
            t.fatal(ets_go::sprintf!("expected 4 nodes, got %d", result.len() as i64));
        }

        let expected_provides = [2usize, 0, 1, 3];
        for (i, &expected) in expected_provides.iter().enumerate() {
            if result[i].provides.len() != expected {
                t.error(ets_go::sprintf!(
                    "result[%d] expected %d provides, got %d",
                    i as i64,
                    expected as i64,
                    result[i].provides.len() as i64
                ));
            }
        }
    });
}

// go test: internal/layergraph/magic_test.go:111 TestDFSPostOrderWithOrderMultipleRoots
#[test]
fn test_dfs_post_order_with_order_multiple_roots() {
    run(|t| {
        t.parallel();
        let mut fx = Fixture::new();
        let mut g = ets_graph::new::<LayerOutlineGraphNodeInfo, ()>();

        let node_a = fx.make_outline_node(3, 0);
        let node_b = fx.make_outline_node(1, 0);

        let idx_a = g.add_node(node_a);
        let idx_b = g.add_node(node_b);

        // Two disconnected roots, sorted by provides desc: A (3) before B (1)
        let result = dfs_post_order_with_order(&g, &[idx_a, idx_b]);

        if result.len() != 2 {
            t.fatal(ets_go::sprintf!("expected 2 nodes, got %d", result.len() as i64));
        }
        // Post-order with A first in stack: A emitted, then B
        // But roots are sorted by provides desc in the caller, so A (3) is first root
        // Post-order of single nodes: just emit them
        // Start order is reversed on stack, so B pushed first, A on top
        // A popped first -> emitted first, then B
        if result[0].provides.len() != 3 {
            t.error(ets_go::sprintf!(
                "expected first node to have 3 provides, got %d",
                result[0].provides.len() as i64
            ));
        }
        if result[1].provides.len() != 1 {
            t.error(ets_go::sprintf!(
                "expected second node to have 1 provides, got %d",
                result[1].provides.len() as i64
            ));
        }
    });
}

// go test: internal/layergraph/magic_test.go:140 TestConvertOutlineGraphToLayerMagicOrdersUnrelatedLayers
#[test]
fn test_convert_outline_graph_to_layer_magic_orders_unrelated_layers() {
    run(|t| {
        t.parallel();
        let mut fx = Fixture::new();
        let mut g = ets_graph::new::<LayerOutlineGraphNodeInfo, ()>();

        let both_provides_and_requires = fx.make_outline_node(1, 1);
        let only_provides = fx.make_outline_node(1, 0);
        let no_provides_or_requires = fx.make_outline_node(0, 0);
        let only_requires = fx.make_outline_node(0, 1);

        // Add nodes in an order that differs from the expected composition order to
        // verify unrelated roots are sorted by their layer shape, not source order.
        g.add_node(no_provides_or_requires.clone());
        g.add_node(only_requires.clone());
        g.add_node(only_provides.clone());
        g.add_node(both_provides_and_requires.clone());

        // Go passes a nil parser and nil target types.
        let tp = fx.type_parser();
        let result = convert_outline_graph_to_layer_magic(&tp, &g, &[]);
        if result.nodes.len() != 4 {
            t.fatal(ets_go::sprintf!("expected 4 nodes, got %d", result.nodes.len() as i64));
        }

        let expected =
            [both_provides_and_requires, only_provides, no_provides_or_requires, only_requires];
        for (i, expected_node) in expected.iter().enumerate() {
            if result.nodes[i].node != expected_node.node {
                t.fatal(ets_go::sprintf!(
                    "result[%d] expected provides=%d requires=%d, got provides=%d requires=%d",
                    i as i64,
                    expected_node.provides.len() as i64,
                    expected_node.requires.len() as i64,
                    result.nodes[i].provided_types.len() as i64,
                    result.nodes[i].required_types.len() as i64
                ));
            }
        }
    });
}

// go test: internal/layergraph/magic_test.go:180 TestNeighborsOutgoingSorted
#[test]
fn test_neighbors_outgoing_sorted() {
    run(|t| {
        t.parallel();
        let mut fx = Fixture::new();
        let mut g = ets_graph::new::<LayerOutlineGraphNodeInfo, ()>();

        let node_a = fx.make_outline_node(0, 0);
        let node_b = fx.make_outline_node(1, 0);
        let node_c = fx.make_outline_node(3, 0);
        let node_d = fx.make_outline_node(2, 0);

        let idx_a = g.add_node(node_a);
        let idx_b = g.add_node(node_b);
        let idx_c = g.add_node(node_c);
        let idx_d = g.add_node(node_d);

        g.add_edge(idx_a, idx_b, ());
        g.add_edge(idx_a, idx_c, ());
        g.add_edge(idx_a, idx_d, ());

        let neighbors = neighbors_outgoing_sorted(&g, idx_a);

        // Should be sorted by provides desc: C (3), D (2), B (1)
        if neighbors.len() != 3 {
            t.fatal(ets_go::sprintf!("expected 3 neighbors, got %d", neighbors.len() as i64));
        }
        if neighbors[0] != idx_c {
            t.error(ets_go::sprintf!(
                "expected first neighbor to be C (3 provides), got index %d",
                neighbors[0]
            ));
        }
        if neighbors[1] != idx_d {
            t.error(ets_go::sprintf!(
                "expected second neighbor to be D (2 provides), got index %d",
                neighbors[1]
            ));
        }
        if neighbors[2] != idx_b {
            t.error(ets_go::sprintf!(
                "expected third neighbor to be B (1 provides), got index %d",
                neighbors[2]
            ));
        }
    });
}
