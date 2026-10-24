// Package typeparser provides Effect type detection and parsing utilities.

use std::sync::Arc;

use ets_graph::{Graph, MermaidOptions, NodeIndex};
use ets_readonly::Slice;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::checker::Type;
use ets_shim::core::P;
use rustc_hash::FxHashMap;

use crate::*;

// go: internal/typeparser/execution_flow.go:12 ExecutionNodeKind
// go: internal/typeparser/execution_flow.go:14 ExecutionNodeKindValue
// Rust: Go's `type ExecutionNodeKind string` with its constants, the only values it takes (docs/PORTING.md,
// "Naming").
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ExecutionNodeKind {
    Value,
    Function,
    LogicMerge,
    Transform,
}

// go: internal/typeparser/execution_flow.go:12 ExecutionNodeKind
impl ExecutionNodeKind {
    /// Rust-only: the Go string value (`string(kind)`).
    pub fn as_str(self) -> &'static str {
        match self {
            ExecutionNodeKind::Value => "value",
            ExecutionNodeKind::Function => "function",
            ExecutionNodeKind::LogicMerge => "logicMerge",
            ExecutionNodeKind::Transform => "transform",
        }
    }
}

// go: internal/typeparser/execution_flow.go:21 ExecutionNode
pub struct ExecutionNode {
    pub(crate) kind: ExecutionNodeKind,
    pub(crate) node: P<Node>,
    pub(crate) typ: Option<Lazy<Option<P<Type>>>>,

    // Transform nodes preserve the original AST in Node and optionally expose a
    // normalized callee/args view once the visitor reaches that node.
    pub(crate) callee: Option<P<Node>>,
    pub(crate) args: Option<&'static [P<Node>]>,
}

impl ExecutionNode {
    // go: internal/typeparser/execution_flow.go:32 ExecutionNode.Kind
    pub fn kind(&self) -> ExecutionNodeKind {
        self.kind
    }

    // go: internal/typeparser/execution_flow.go:34 ExecutionNode.Node
    pub fn node(&self) -> P<Node> {
        self.node
    }

    // go: internal/typeparser/execution_flow.go:36 ExecutionNode.Type
    // Rust: the lazy resolves through `tp` (see `Lazy`).
    pub fn type_(&self, tp: &mut TypeParser<'_>) -> Option<P<Type>> {
        Lazy::get(self.typ.as_ref(), tp)
    }

    // go: internal/typeparser/execution_flow.go:38 ExecutionNode.Callee
    pub fn callee(&self) -> Option<P<Node>> {
        self.callee
    }

    // go: internal/typeparser/execution_flow.go:40 ExecutionNode.Args
    pub fn args(&self) -> Slice<'_, P<Node>> {
        ets_readonly::new(self.args)
    }
}

// go: internal/typeparser/execution_flow.go:42 ExecutionLinkKind
// go: internal/typeparser/execution_flow.go:44 ExecutionLinkKindUsedBy
// Rust: Go's `type ExecutionLinkKind string` with its constants, the only values it takes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ExecutionLinkKind {
    UsedBy,
    Pipe,
    PotentialReturn,
    Yieldable,
    Parameter,
    TransformArg,
    TransformCallee,
}

// go: internal/typeparser/execution_flow.go:42 ExecutionLinkKind
impl ExecutionLinkKind {
    /// Rust-only: the Go string value (`string(kind)`).
    pub fn as_str(self) -> &'static str {
        match self {
            ExecutionLinkKind::UsedBy => "usedBy",
            ExecutionLinkKind::Pipe => "pipe",
            ExecutionLinkKind::PotentialReturn => "potentialReturn",
            ExecutionLinkKind::Yieldable => "yieldable",
            ExecutionLinkKind::Parameter => "parameter",
            ExecutionLinkKind::TransformArg => "transformArg",
            ExecutionLinkKind::TransformCallee => "transformCallee",
        }
    }
}

// go: internal/typeparser/execution_flow.go:54 ExecutionLink
pub struct ExecutionLink {
    pub(crate) kind: ExecutionLinkKind,
    pub(crate) node: Option<P<Node>>,
}

impl ExecutionLink {
    // go: internal/typeparser/execution_flow.go:59 ExecutionLink.Kind
    pub fn kind(&self) -> ExecutionLinkKind {
        self.kind
    }

    // go: internal/typeparser/execution_flow.go:61 ExecutionLink.Node
    pub fn node(&self) -> Option<P<Node>> {
        self.node
    }
}

// go: internal/typeparser/execution_flow.go:64 ExecutionFlow
pub struct ExecutionFlow {
    pub(crate) graph: Box<Graph<ExecutionNode, ExecutionLink>>,
}

// go: internal/typeparser/execution_flow.go:67 GraphSlice
// Rust: Go's `*graph.NodeIndex` fields are never nil
// (every slice is built by `buildSlice` or joins two built slices), so they are plain indices; Go's `*GraphSlice`
// is `Option<GraphSlice>` (a copy: nothing writes a slice after it is built).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GraphSlice {
    pub leading: NodeIndex,
    pub trailing: NodeIndex,
}

impl ExecutionFlow {
    // go: internal/typeparser/execution_flow.go:73 ExecutionFlow.ToMermaid
    pub fn to_mermaid(&self, options: MermaidOptions<'_, ExecutionNode, ExecutionLink>) -> String {
        self.graph.to_mermaid(options)
    }
}

// go: internal/typeparser/execution_flow.go:77 executionCollector
// Rust: Go's `*core.LinkStore[*ast.Node, *GraphSlice]` is a map whose entries may hold nil (`Has` tells an entry
// from a missing one).
pub(crate) struct ExecutionCollector<'a, 'c> {
    tp: &'a mut TypeParser<'c>,
    g: Graph<ExecutionNode, ExecutionLink>,
    parsed: FxHashMap<P<Node>, Option<GraphSlice>>,
    usage_target: Option<GraphSlice>,
}

impl ExecutionCollector<'_, '_> {
    // go: internal/typeparser/execution_flow.go:84 executionCollector.buildSlice
    pub(crate) fn build_slice(&mut self, node: ExecutionNode) -> GraphSlice {
        let node_index = self.g.add_node(node);
        GraphSlice { leading: node_index, trailing: node_index }
    }

    // go: internal/typeparser/execution_flow.go:92 executionCollector.buildValueNode
    pub(crate) fn build_value_node(&mut self, node: P<Node>) -> GraphSlice {
        let typ = self.tp.lazy_type_at_location(node);
        self.build_slice(ExecutionNode {
            kind: ExecutionNodeKind::Value,
            node,
            typ: Some(typ),
            callee: None,
            args: None,
        })
    }

    // go: internal/typeparser/execution_flow.go:100 executionCollector.extractCalleeAndArgs
    // Rust: Go's `node.Arguments()` of a call is the call's own argument list.
    #[expect(clippy::self_only_used_in_recursion, reason = "a collector method in Go")]
    pub(crate) fn extract_callee_and_args(
        &self,
        node: P<Node>,
    ) -> (Option<P<Node>>, Option<&'static [P<Node>]>) {
        if ets_shim::ast::is_parenthesized_expression(node) {
            return self
                .extract_callee_and_args(node.expression().expect("a parenthesized expression"));
        } else if ets_shim::ast::is_call_expression(node) {
            return (node.expression(), Some(node.arguments()));
        }
        (Some(node), None)
    }

    // go: internal/typeparser/execution_flow.go:109 executionCollector.connectSlices
    pub(crate) fn connect_slices(
        &mut self,
        from_slice: Option<GraphSlice>,
        to_slice: Option<GraphSlice>,
        kind: ExecutionLinkKind,
    ) -> Option<GraphSlice> {
        let Some(from_slice) = from_slice else {
            return to_slice;
        };
        let Some(to_slice) = to_slice else {
            return Some(from_slice);
        };
        if from_slice.trailing == to_slice.leading {
            return Some(GraphSlice { leading: from_slice.leading, trailing: to_slice.trailing });
        }
        self.g.add_edge(from_slice.trailing, to_slice.leading, ExecutionLink { kind, node: None });
        Some(GraphSlice { leading: from_slice.leading, trailing: to_slice.trailing })
    }

    // go: internal/typeparser/execution_flow.go:131 executionCollector.visitNode
    pub(crate) fn visit_node(&mut self, node: Option<P<Node>>) -> Option<GraphSlice> {
        // avoid double traversal
        let node = node?;
        if let Some(&s) = self.parsed.get(&node) {
            return s;
        }

        let previous_usage_target = self.usage_target;
        self.usage_target = None;

        // actual visit logic
        let s = if let Some(parsed_effect_gen) = self.tp.effect_gen_call(node) {
            self.visit_effect_gen_call(&parsed_effect_gen, node)
        } else if let Some(parsed_effect_fn) = self.tp.effect_fn_call(node) {
            self.visit_effect_fn_call(&parsed_effect_fn, node)
        } else if let Some(parsed_pipe_call) = self.tp.parse_pipe_call(node) {
            self.visit_pipe_call(&parsed_pipe_call, node)
        } else if let Some(parsed_single_arg) = self.tp.single_arg_inline_call(Some(node)) {
            self.visit_single_arg_inline_call(&parsed_single_arg, node)
        } else if let Some(parsed_data_first_or_last) = self.tp.data_first_or_last_call(node) {
            self.visit_data_first_or_last_call(&parsed_data_first_or_last, node)
        } else if ets_shim::ast::is_function_like_declaration(node) {
            self.visit_function_like_declaration(node)
        } else {
            self.visit_expression_node(node, previous_usage_target)
        };
        // store to avoid double-traversal
        self.parsed.insert(node, s);
        self.usage_target = previous_usage_target;

        s
    }

    // go: internal/typeparser/execution_flow.go:167 executionCollector.visitNodesAndConnectSlice
    pub(crate) fn visit_nodes_and_connect_slice(
        &mut self,
        nodes: &[P<Node>],
        target_slice: Option<GraphSlice>,
        kind: ExecutionLinkKind,
    ) -> bool {
        for &n in nodes {
            self.visit_node_and_connect_slice(Some(n), target_slice, kind);
        }
        false
    }

    // go: internal/typeparser/execution_flow.go:174 executionCollector.visitNodeAndConnectSlice
    pub(crate) fn visit_node_and_connect_slice(
        &mut self,
        n: Option<P<Node>>,
        target_slice: Option<GraphSlice>,
        kind: ExecutionLinkKind,
    ) -> bool {
        let node_slice = self.visit_node(n);
        self.connect_slices(node_slice, target_slice, kind);
        false
    }

    // go: internal/typeparser/execution_flow.go:180 executionCollector.visitNodeVisitorConnectUsageTarget
    pub(crate) fn visit_node_visitor_connect_usage_target(&mut self, node: P<Node>) -> bool {
        let s = self.visit_node(Some(node));
        let usage_target = self.usage_target;
        self.connect_slices(s, usage_target, ExecutionLinkKind::UsedBy);
        false
    }

    // go: internal/typeparser/execution_flow.go:189 executionCollector.visitEachChildWithUsageTarget
    pub(crate) fn visit_each_child_with_usage_target(
        &mut self,
        node: Option<P<Node>>,
        target: Option<GraphSlice>,
    ) -> bool {
        let Some(node) = node else {
            return false;
        };
        let previous = self.usage_target;
        self.usage_target = target;
        node.for_each_child(&mut |child| self.visit_node_visitor_connect_usage_target(child));
        self.usage_target = previous;
        false
    }

    // go: internal/typeparser/execution_flow.go:200 executionCollector.visitExpressionNode
    // Rust: Go compares the `*GraphSlice` pointers `rootExpr != parentExpression`, which holds exactly when a value
    // node was built here.
    pub(crate) fn visit_expression_node(
        &mut self,
        node: P<Node>,
        parent_expression: Option<GraphSlice>,
    ) -> Option<GraphSlice> {
        let mut root_expr = parent_expression;
        let mut built = false;
        if parent_expression.is_none()
            && ets_shim::ast::is_expression_node(node)
            && !is_inside_type_only_heritage_expression(node)
        {
            root_expr = Some(self.build_value_node(node));
            built = true;
        }
        self.visit_each_child_with_usage_target(Some(node), root_expr);
        if built {
            return root_expr;
        }
        None
    }

    // go: internal/typeparser/execution_flow.go:212 executionCollector.visitPipeCall
    pub(crate) fn visit_pipe_call(
        &mut self,
        p: &Arc<ParsedPipeCallResult>,
        _node: P<Node>,
    ) -> Option<GraphSlice> {
        let mut s = self.visit_node(Some(p.subject));
        for (i, &piped_transform) in p.args.iter().enumerate() {
            let (callee, args) = self.extract_callee_and_args(piped_transform);
            let shared = Arc::clone(p);
            let transform_slice = self.build_slice(ExecutionNode {
                kind: ExecutionNodeKind::Transform,
                node: piped_transform,
                typ: Some(new_lazy(move |tp| Lazy::get(Some(&shared.args_out_type), tp)[i])),
                callee,
                args,
            });
            self.parsed.insert(piped_transform, Some(transform_slice));
            self.visit_node_and_connect_slice(
                callee,
                Some(transform_slice),
                ExecutionLinkKind::TransformCallee,
            );
            self.visit_nodes_and_connect_slice(
                args.unwrap_or_default(),
                Some(transform_slice),
                ExecutionLinkKind::TransformArg,
            );
            s = self.connect_slices(s, Some(transform_slice), ExecutionLinkKind::Pipe);
        }
        s
    }

    // go: internal/typeparser/execution_flow.go:231 executionCollector.visitEffectGenCall
    pub(crate) fn visit_effect_gen_call(
        &mut self,
        p: &EffectGenCallResult,
        node: P<Node>,
    ) -> Option<GraphSlice> {
        let typ = self.tp.lazy_type_at_location(node);
        let s = Some(self.build_slice(ExecutionNode {
            kind: ExecutionNodeKind::LogicMerge,
            node,
            typ: Some(typ),
            callee: None,
            args: None,
        }));
        if let Some(body) = p.body {
            ets_shim::ast::for_each_return_statement(body, |stmt| {
                if stmt.kind() == Kind::ReturnStatement {
                    self.visit_node_and_connect_slice(
                        stmt.expression(),
                        s,
                        ExecutionLinkKind::PotentialReturn,
                    );
                }
                false
            });
            ets_shim::checker::for_each_yield_expression(body, |expr| {
                if let Some(expression) = expr.expression() {
                    self.parsed.insert(expr, None);
                    self.visit_node_and_connect_slice(
                        Some(expression),
                        s,
                        ExecutionLinkKind::Yieldable,
                    );
                }
                false
            });
        }
        if p.body.is_some() {
            self.visit_each_child_with_usage_target(p.body, s);
        }
        s
    }

    // go: internal/typeparser/execution_flow.go:256 executionCollector.visitEffectFnCall
    pub(crate) fn visit_effect_fn_call(
        &mut self,
        p: &EffectFnCallResult,
        node: P<Node>,
    ) -> Option<GraphSlice> {
        let mut s_exit = Some(self.build_slice(ExecutionNode {
            kind: ExecutionNodeKind::LogicMerge,
            node: p.function_node,
            typ: Some(p.function_return_type.clone()),
            callee: None,
            args: None,
        }));
        for (i, &piped_transform) in p.pipe_arguments.unwrap_or_default().iter().enumerate() {
            let (callee, args) = self.extract_callee_and_args(piped_transform);
            let transform_slice = self.build_slice(ExecutionNode {
                kind: ExecutionNodeKind::Transform,
                node: piped_transform,
                typ: Some(p.pipe_args_out_type[i].clone()), // TODO: OOB?
                callee,
                args,
            });
            self.parsed.insert(piped_transform, Some(transform_slice));
            self.visit_node_and_connect_slice(
                callee,
                Some(transform_slice),
                ExecutionLinkKind::TransformCallee,
            );
            self.visit_nodes_and_connect_slice(
                args.unwrap_or_default(),
                Some(transform_slice),
                ExecutionLinkKind::TransformArg,
            );
            s_exit = self.connect_slices(s_exit, Some(transform_slice), ExecutionLinkKind::Pipe);
        }
        if p.is_generator()
            && let Some(body) = p.body()
        {
            ets_shim::checker::for_each_yield_expression(body, |expr| {
                if let Some(expression) = expr.expression() {
                    self.parsed.insert(expr, None);
                    self.visit_node_and_connect_slice(
                        Some(expression),
                        s_exit,
                        ExecutionLinkKind::Yieldable,
                    );
                }
                false
            });
        }
        if p.body().is_some_and(ets_shim::ast::is_expression_node) {
            self.visit_node_and_connect_slice(p.body(), s_exit, ExecutionLinkKind::Pipe);
        } else if let Some(body) = p.body() {
            ets_shim::ast::for_each_return_statement(body, |stmt| {
                if stmt.kind() == Kind::ReturnStatement {
                    self.visit_node_and_connect_slice(
                        stmt.expression(),
                        s_exit,
                        ExecutionLinkKind::PotentialReturn,
                    );
                }
                false
            });
            self.visit_each_child_with_usage_target(Some(body), s_exit);
        }
        // function with parameters
        let typ = self.tp.lazy_type_at_location(node);
        let s = Some(self.build_slice(ExecutionNode {
            kind: ExecutionNodeKind::Function,
            node,
            typ: Some(typ),
            callee: None,
            args: None,
        }));
        for &arg in p.function_node.parameters() {
            self.visit_node_and_connect_slice(Some(arg), s, ExecutionLinkKind::Parameter);
        }
        self.connect_slices(s_exit, s, ExecutionLinkKind::PotentialReturn);
        s
    }

    // go: internal/typeparser/execution_flow.go:309 executionCollector.visitSingleArgInlineCall
    pub(crate) fn visit_single_arg_inline_call(
        &mut self,
        p: &ParsedSingleArgInlineCallTransform,
        node: P<Node>,
    ) -> Option<GraphSlice> {
        let mut s = self.visit_node(Some(p.subject));
        let (callee, args) = self.extract_callee_and_args(p.transform);
        let typ = self.tp.lazy_type_at_location(node);
        let transform_slice = self.build_slice(ExecutionNode {
            kind: ExecutionNodeKind::Transform,
            node: p.transform,
            typ: Some(typ),
            callee,
            args,
        });
        s = self.connect_slices(s, Some(transform_slice), ExecutionLinkKind::Pipe);
        self.visit_node_and_connect_slice(
            callee,
            Some(transform_slice),
            ExecutionLinkKind::TransformCallee,
        );
        self.visit_nodes_and_connect_slice(
            args.unwrap_or_default(),
            Some(transform_slice),
            ExecutionLinkKind::TransformArg,
        );
        s
    }

    // go: internal/typeparser/execution_flow.go:325 executionCollector.visitDataFirstOrLastCall
    pub(crate) fn visit_data_first_or_last_call(
        &mut self,
        p: &ParsedDataFirstOrLastCall,
        node: P<Node>,
    ) -> Option<GraphSlice> {
        let mut s = self.visit_node(Some(p.subject));
        let typ = self.tp.lazy_type_at_location(node);
        let transform_slice = self.build_slice(ExecutionNode {
            kind: ExecutionNodeKind::Transform,
            node,
            typ: Some(typ),
            callee: None,
            args: None,
        });
        s = self.connect_slices(s, Some(transform_slice), ExecutionLinkKind::Pipe);
        self.visit_node_and_connect_slice(Some(p.callee), s, ExecutionLinkKind::TransformCallee);
        self.visit_nodes_and_connect_slice(&p.args, s, ExecutionLinkKind::TransformArg);
        s
    }

    // go: internal/typeparser/execution_flow.go:338 executionCollector.visitFunctionLikeDeclaration
    pub(crate) fn visit_function_like_declaration(&mut self, node: P<Node>) -> Option<GraphSlice> {
        let typ = self.tp.lazy_type_at_location(node);
        let s = Some(self.build_slice(ExecutionNode {
            kind: ExecutionNodeKind::Function,
            node,
            typ: Some(typ),
            callee: None,
            args: None,
        }));
        self.visit_nodes_and_connect_slice(node.parameters(), s, ExecutionLinkKind::Parameter);
        if let Some(fn_body) = node.body() {
            if ets_shim::ast::is_expression_node(fn_body) {
                self.visit_node_and_connect_slice(
                    Some(fn_body),
                    s,
                    ExecutionLinkKind::PotentialReturn,
                );
            } else {
                ets_shim::ast::for_each_return_statement(fn_body, |stmt| {
                    if stmt.kind() == Kind::ReturnStatement {
                        self.visit_node_and_connect_slice(
                            stmt.expression(),
                            s,
                            ExecutionLinkKind::PotentialReturn,
                        );
                    }
                    false
                });
                self.visit_each_child_with_usage_target(Some(fn_body), s);
            }
        }
        s
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/execution_flow.go:362 TypeParser.ExecutionFlow
    pub fn execution_flow(
        &mut self,
        sf: impl Into<Option<P<SourceFile>>>,
    ) -> Option<Arc<ExecutionFlow>> {
        let sf = sf.into()?;

        cached(
            self,
            |l| &mut l.execution_flow,
            sf,
            |tp| {
                let g = ets_graph::new::<ExecutionNode, ExecutionLink>();
                let mut ec =
                    ExecutionCollector { tp, g, parsed: FxHashMap::default(), usage_target: None };
                ec.visit_node(Some(sf.as_node()));
                Some(Arc::new(ExecutionFlow { graph: Box::new(ec.g) }))
            },
        )
    }
}

// go: internal/typeparser/execution_flow.go:379 parsedSingleArgInlineCallTransform
pub(crate) struct ParsedSingleArgInlineCallTransform {
    subject: P<Node>,
    transform: P<Node>,
}

impl TypeParser<'_> {
    // go: internal/typeparser/execution_flow.go:384 TypeParser.singleArgInlineCall
    pub(crate) fn single_arg_inline_call(
        &mut self,
        node: Option<P<Node>>,
    ) -> Option<ParsedSingleArgInlineCallTransform> {
        let node = node?;
        if node.kind() != Kind::CallExpression {
            return None;
        }
        let outer_call_expr = node.as_call_expression();
        let outer_call_args = node.arguments();
        if outer_call_args.len() != 1 {
            return None;
        }
        let called_expr_type = self.get_type_at_location(outer_call_expr.expression())?;
        let call_sigs = self.checker.get_call_signatures(called_expr_type);
        if call_sigs.len() != 1 {
            return None;
        }
        let params = call_sigs[0].parameters();
        if params.len() != 1 {
            return None;
        }

        Some(ParsedSingleArgInlineCallTransform {
            subject: outer_call_args[0],
            transform: outer_call_expr.expression(),
        })
    }
}
