// Rust: Go's tests build outline nodes from synthetic `&ast.Node{Kind: ast.KindCallExpression}` nodes and nil
// `*checker.Type` slots. A `P<Node>` or `P<Type>` cannot be null, so `Fixture` compiles a small program whose call
// expressions stand for the synthetic nodes and whose one type fills the slots (only the slice lengths matter).

use std::sync::Arc;

use ets_go::testing::{self, T};
use ets_graph::NodeIndex;
use ets_shim::ast::{Kind, Node};
use ets_shim::checker::Type;
use ets_shim::compiler::{CheckerHandle, ProgramOptions};
use ets_shim::core::context::Context;
use ets_shim::core::tspath::ComparePathsOptions;
use ets_shim::core::{CompilerOptions, P, ScriptTarget, Tristate};
use ets_shim::vfs::{FS, bundled, vfstest};

use crate::*;

pub(crate) fn run(f: impl FnOnce(&T) + Send + 'static) {
    ets_go::testing::on_checker_stack(move || testing::run(f));
}

/// Rust-only: the program standing in for Go's synthetic nodes and nil types (see the file comment).
pub(crate) struct Fixture {
    _checker: CheckerHandle,
    pub(crate) calls: Vec<P<Node>>,
    next_call: usize,
    pub(crate) dummy: P<Type>,
}

impl Fixture {
    pub(crate) fn new() -> Fixture {
        let source = "declare const f: () => void;\nf(); f(); f(); f(); f(); f(); f(); f();\n";
        let fs: Arc<dyn FS> =
            Arc::new(bundled::wrap_fs(vfstest::from_map([("/.src/test.ts", source)], true)));
        let compiler_options = P::new(CompilerOptions {
            target: ScriptTarget::ES_NEXT,
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
        Fixture { _checker: checker, calls, next_call: 0, dummy }
    }

    // go test: internal/layergraph/magic_test.go:85 makeOutlineNode 6715734ad40e8109
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

// go test: internal/layergraph/magic_test.go:11 TestCompareNodesByOrder bc6e7291b41b23df
#[test]
fn test_compare_nodes_by_order() {
    run(|t| {
        t.parallel();
        let mut fx = Fixture::new();
        let mut g = ets_graph::new::<LayerOutlineGraphNodeInfo, ()>();
        let ordered: [NodeIndex; 7] = [
            g.add_node(fx.make_outline_node(3, 3)),
            g.add_node(fx.make_outline_node(3, 1)),
            g.add_node(fx.make_outline_node(1, 2)),
            g.add_node(fx.make_outline_node(3, 0)),
            g.add_node(fx.make_outline_node(1, 0)),
            g.add_node(fx.make_outline_node(0, 0)),
            g.add_node(fx.make_outline_node(0, 1)),
        ];
        for i in 0..ordered.len() - 1 {
            let cmp = compare_nodes_by_order(&g, ordered[i], ordered[i + 1]);
            if cmp >= 0 {
                t.error(ets_go::sprintf!(
                    "expected node %d to sort before node %d, got %d",
                    i as i64,
                    (i + 1) as i64,
                    cmp
                ));
            }
            let cmp = compare_nodes_by_order(&g, ordered[i + 1], ordered[i]);
            if cmp <= 0 {
                t.error(ets_go::sprintf!(
                    "expected node %d to sort after node %d, got %d",
                    (i + 1) as i64,
                    i as i64,
                    cmp
                ));
            }
        }
    });
}

// go test: internal/layergraph/magic_test.go:33 TestDFSPostOrderWithOrder 16746ce2b5f0e4f6
#[test]
fn test_dfs_post_order_with_order() {
    run(|t| {
        t.parallel();
        let mut fx = Fixture::new();
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

// go test: internal/layergraph/magic_test.go:65 TestDFSPostOrderWithOrderKeepsRootOrder d1281a027fff0a98
#[test]
fn test_dfs_post_order_with_order_keeps_root_order() {
    run(|t| {
        t.parallel();
        let mut fx = Fixture::new();
        let mut g = ets_graph::new::<LayerOutlineGraphNodeInfo, ()>();

        let node_a = fx.make_outline_node(3, 0);
        let node_b = fx.make_outline_node(1, 0);

        let idx_a = g.add_node(node_a);
        let idx_b = g.add_node(node_b);

        let result = dfs_post_order_with_order(&g, &[idx_b, idx_a]);

        if result.len() != 2 {
            t.fatal(ets_go::sprintf!("expected 2 nodes, got %d", result.len() as i64));
        }
        if result[0].provides.len() != 1 || result[1].provides.len() != 3 {
            t.error(ets_go::sprintf!(
                "expected roots in the given order (1 then 3 provides), got %d then %d",
                result[0].provides.len() as i64,
                result[1].provides.len() as i64
            ));
        }
    });
}
